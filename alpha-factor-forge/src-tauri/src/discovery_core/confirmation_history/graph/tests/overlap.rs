use super::*;
fn fork_validation(offset: u64, keychar: char) -> (ConfirmationInventory, GraphContext) {
    let (mut i, mut c) = val_only();
    let mut b = doc(&i.batches[0]);
    for name in ["trainWindow", "validationWindow", "testWindow"] {
        for edge in ["fromMs", "toExclusiveMs"] {
            let n = b["body"]["split"][name][edge].as_u64().unwrap();
            b["body"]["split"][name][edge] = json!(n + offset);
        }
    }
    for name in ["validationSupport", "testSupport"] {
        for v in b["body"]["split"][name].as_array_mut().unwrap() {
            for edge in ["fromMs", "toExclusiveMs"] {
                v[edge] = json!(v[edge].as_u64().unwrap() + offset);
            }
        }
    }
    rehash(&mut b);
    i.batches[0] = b.to_string();
    let mut op = doc(&i.operations[0]);
    op["body"]["confirmationBatchId"] = b["id"].clone();
    op["body"]["reservationKey"] = json!(keychar.to_string().repeat(32));
    rehash(&mut op);
    i.operations[0] = op.to_string();
    let mut alpha = c.reservations[0].document();
    alpha["body"]["confirmationBatchId"] = b["id"].clone();
    alpha["body"]["reservationKey"] = op["body"]["reservationKey"].clone();
    alpha["reservationId"] =
        json!(identity("research-alpha-reservation-v1", &alpha["body"]).unwrap());
    let alpha = parse_alpha_reservation(&alpha.to_string()).unwrap();
    let mut exposure = doc(&i.exposures[0]);
    exposure["body"]["reservationKey"] = op["body"]["reservationKey"].clone();
    exposure["body"]["intervals"] = b["body"]["split"]["validationSupport"].clone();
    exposure["body"]["claim"]["operationId"] = op["id"].clone();
    exposure["body"]["claim"]["alphaReservationId"] = json!(alpha.reservation_id());
    rehash(&mut exposure);
    i.exposures[0] = exposure.to_string();
    change_admission(&mut i, |a| {
        a["operationId"] = op["id"].clone();
        a["alphaReservationId"] = json!(alpha.reservation_id());
        a["exposureId"] = exposure["id"].clone();
    });
    let mut start = doc(&i.events[0]);
    start["body"]["operationId"] = op["id"].clone();
    rehash(&mut start);
    i.events[0] = start.to_string();
    let mut seal = doc(&i.events[1]);
    seal["body"]["operationId"] = op["id"].clone();
    seal["body"]["event"]["executionStartedId"] = start["id"].clone();
    rehash(&mut seal);
    i.events[1] = seal.to_string();
    c.reservations = vec![alpha];
    validate_confirmation_graph(&i, &c).unwrap();
    (i, c)
}
fn union(
    i: &mut ConfirmationInventory,
    c: &mut GraphContext,
    other: ConfirmationInventory,
    host: GraphContext,
) {
    for (section, extra) in [
        (&mut i.batches, other.batches),
        (&mut i.operations, other.operations),
        (&mut i.exposures, other.exposures),
        (&mut i.admissions, other.admissions),
        (&mut i.events, other.events),
    ] {
        for raw in extra {
            if !section.iter().any(|s| doc(s)["id"] == doc(&raw)["id"]) {
                section.push(raw);
            }
        }
    }
    for b in other.blobs {
        if !i.blobs.iter().any(|v| v.sha256() == b.sha256()) {
            i.blobs.push(b);
        }
    }
    for a in host.reservations {
        if !c
            .reservations
            .iter()
            .any(|v| v.reservation_id() == a.reservation_id())
        {
            c.reservations.push(a);
        }
    }
}
#[test]
fn sweep_proves_real_edges_for_nontransitive_chains_and_touching_endpoints() {
    let (mut i, mut c) = val_only();
    let support = &doc(&i.exposures[0])["body"]["intervals"][0];
    let length = support["toExclusiveMs"].as_u64().unwrap() - support["fromMs"].as_u64().unwrap();
    let (b, h) = fork_validation(length - 1, '5');
    union(&mut i, &mut c, b, h);
    let (b, h) = fork_validation(2 * (length - 1), '6');
    union(&mut i, &mut c, b, h);
    let completed = complete_confirmation_graph(&i, &c).unwrap();
    assert_eq!(completed.inventory().conflicts.len(), 2);
    for raw in &completed.inventory().conflicts {
        assert_eq!(doc(raw)["body"]["kind"], "exposure_overlap");
    }
    validate_confirmation_graph(completed.inventory(), &c).unwrap();
    let mut reversed = i.clone();
    reversed.exposures.reverse();
    assert_eq!(
        complete_confirmation_graph(&reversed, &c)
            .unwrap()
            .inventory(),
        completed.inventory()
    );
    let a = doc(&i.exposures[0])["id"].as_str().unwrap().to_string();
    let z = doc(&i.exposures[2])["id"].as_str().unwrap().to_string();
    let checked_nodes = nodes(&i, None).unwrap();
    let mut checked = Checked::default();
    for b in &i.blobs {
        checked.blobs.insert(b.sha256().into(), b.clone());
    }
    let host = Host::new(&c, None).unwrap();
    for k in topological(&checked_nodes).unwrap() {
        checked.check(&k, &checked_nodes[&k], &host).unwrap();
    }
    let false_edge = checked
        .conflict(ConflictKind::ExposureOverlap, &a, &z)
        .unwrap();
    i.conflicts.push(false_edge.document().to_string());
    assert_bad(&i, &c);
    let (mut i, mut c) = val_only();
    let (b, h) = fork_validation(length, '5');
    union(&mut i, &mut c, b, h);
    assert!(complete_confirmation_graph(&i, &c)
        .unwrap()
        .inventory()
        .conflicts
        .is_empty());
}
#[test]
fn manual_exemption_is_specific_and_old_valid_witnesses_survive() {
    let (mut i, mut c) = val_only();
    let seal = doc(&i.events[1])["id"].as_str().unwrap().to_string();
    let start = doc(&i.exposures[0])["body"]["intervals"][0]["fromMs"]
        .as_u64()
        .unwrap();
    manual(&mut i, '7', start + 1, start + 2, Some(seal));
    let (b, h) = fork_validation(0, '5');
    union(&mut i, &mut c, b, h);
    let completed = complete_confirmation_graph(&i, &c).unwrap();
    assert!(completed
        .inventory()
        .conflicts
        .iter()
        .any(|s| doc(s)["body"]["kind"] == "manual_overlap"));
    let mut i = completed.inventory().clone();
    let old = i.conflicts.clone();
    let (b, h) = fork_validation(0, '6');
    union(&mut i, &mut c, b, h);
    let completed = complete_confirmation_graph(&i, &c).unwrap();
    assert!(old
        .iter()
        .all(|s| completed.inventory().conflicts.contains(s)));
    validate_confirmation_graph(completed.inventory(), &c).unwrap();
}
