use math_core::{Atom, EditError, Model, NodeKind, Slot, Span};

#[test]
fn inserting_and_removing_a_node_reuses_the_slot() {
    let (mut model, root) = Model::build(|m| {
        let a = m.symbol('a');
        let b = m.symbol('b');
        m.row(vec![a, b])
    });
    assert_eq!(model.len(), 3);

    let plus = model
        .insert(
            root,
            Slot::Body,
            1,
            NodeKind::Atom(Atom::new('+')),
            Span::default(),
        )
        .expect("a row accepts a body insertion");
    assert_eq!(model.len(), 4);
    let NodeKind::Row(children) = model.kind(root) else {
        panic!("expected a row");
    };
    assert_eq!(children, &vec![children[0], plus, children[2]]);

    model.remove(plus).expect("the inserted node detaches");
    assert!(!model.is_alive(plus));
    assert_eq!(model.len(), 3);

    // The freed slot is reused, so old ids must not be trusted after removal.
    let again = model
        .insert(
            root,
            Slot::Body,
            1,
            NodeKind::Atom(Atom::new('-')),
            Span::default(),
        )
        .expect("the row still accepts an insertion");
    assert_eq!(again, plus);
}

#[test]
fn replacing_a_node_keeps_the_subtrees_it_still_refers_to() {
    let (mut model, root) = Model::build(|m| {
        let a = m.symbol('a');
        let numerator = m.row(vec![a]);
        let b = m.symbol('b');
        let denominator = m.row(vec![b]);
        m.fraction(numerator, denominator, true)
    });
    let NodeKind::Fraction {
        numerator,
        denominator,
        ..
    } = model.kind(root).clone()
    else {
        panic!("expected a fraction");
    };
    let NodeKind::Row(numerator_children) = model.kind(numerator).clone() else {
        panic!("expected a row");
    };
    let a = numerator_children[0];

    model
        .replace(root, NodeKind::Row(vec![numerator]))
        .expect("the numerator is reusable in the new row");

    assert!(model.is_alive(numerator) && model.is_alive(a));
    assert!(!model.is_alive(denominator));
    assert_eq!(
        model.len(),
        3,
        "only the root and the kept numerator row remain"
    );
}

#[test]
fn set_root_frees_the_old_tree_and_refuses_a_stale_root() {
    let (mut model, root) = Model::build(|m| {
        let a = m.symbol('a');
        let b = m.symbol('b');
        m.row(vec![a, b])
    });
    let NodeKind::Row(children) = model.kind(root) else {
        panic!("expected a row");
    };
    let (a, b) = (children[0], children[1]);
    let new_root = model
        .insert(
            root,
            Slot::Body,
            0,
            NodeKind::Row(Vec::new()),
            Span::default(),
        )
        .expect("the old root accepts a new subtree");

    model.set_root(new_root).expect("the new root is live");
    assert!(model.is_alive(new_root));
    assert!(!model.is_alive(root), "the old root is unreachable");
    assert!(!model.is_alive(a) && !model.is_alive(b));
    assert_eq!(model.len(), 1);
    assert!(model.try_kind(root).is_none());

    assert_eq!(model.set_root(root), Err(EditError::NoSuchNode));
}

#[test]
fn removing_a_required_child_is_refused() {
    let (mut model, root) = Model::build(|m| {
        let a = m.symbol('a');
        let numerator = m.row(vec![a]);
        let b = m.symbol('b');
        let denominator = m.row(vec![b]);
        m.fraction(numerator, denominator, true)
    });
    let NodeKind::Fraction { numerator, .. } = model.kind(root) else {
        panic!("expected a fraction");
    };
    let numerator = *numerator;
    assert_eq!(model.remove(numerator), Err(EditError::RequiredChild));
    assert!(model.is_alive(numerator));
}
