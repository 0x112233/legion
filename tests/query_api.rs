use hydro::*;
use std::collections::HashMap;
// use std::iter::FromIterator;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pos(f32, f32, f32);
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rot(f32, f32, f32);
#[derive(Clone, Copy, Debug, PartialEq)]
struct Scale(f32, f32, f32);
#[derive(Clone, Copy, Debug, PartialEq)]
struct Vel(f32, f32, f32);
#[derive(Clone, Copy, Debug, PartialEq)]
struct Accel(f32, f32, f32);
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct Model(u32);
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct Static;

// fn create_test_world() -> (
//     World,
//     HashMap<
//         Entity,
//         (
//             Option<Pos>,
//             Option<Rot>,
//             Option<Vel>,
//             Option<Model>,
//             Option<Static>,
//         ),
//     >,
// ) {
//     let universe = Universe::new(None);
//     let mut world = universe.create_world();
//     let mut expected: HashMap<
//         Entity,
//         (
//             Option<Pos>,
//             Option<Rot>,
//             Option<Vel>,
//             Option<Model>,
//             Option<Static>,
//         ),
//     > = HashMap::new();

// // pos, rot
// let data = Vec::from_iter(std::iter::unfold(0f32, |x| {*x += 1.; Some((Pos(*x + 1., *x + 1., *x + 2.), Rot(*x + 3., *x + 4., *x + 5.))) }).take(1000));
// for (i, e) in world.insert_from((), data.clone()).iter().enumerate() {
//     let (pos, rot) = data.get(i).unwrap();
//     expected.insert(*e, (Some(*pos), Some(*rot), None, None, None));
// }

// // model(1) | pos, rot
// let data = Vec::from_iter(std::iter::unfold(0f32, |x| {*x += 1.; Some((Pos(*x + 1., *x + 1., *x + 2.), Rot(*x + 3., *x + 4., *x + 5.))) }).take(1000));
// for (i, e) in world.insert_from((Model(1),), data.clone()).iter().enumerate() {
//     let (pos, rot) = data.get(i).unwrap();
//     expected.insert(*e, (Some(*pos), Some(*rot), None, Some(Model(1)), None));
// }

// // model(2) | pos, rot
// let data = Vec::from_iter(std::iter::unfold(0f32, |x| {*x += 1.; Some((Pos(*x + 1., *x + 1., *x + 2.), Rot(*x + 3., *x + 4., *x + 5.))) }).take(1000));
// for (i, e) in world.insert_from((Model(2),), data.clone()).iter().enumerate() {
//     let (pos, rot) = data.get(i).unwrap();
//     expected.insert(*e, (Some(*pos), Some(*rot), None, Some(Model(2)), None));
// }

// // static | pos, rot
// let data = Vec::from_iter(std::iter::unfold(0f32, |x| {*x += 1.; Some((Pos(*x + 1., *x + 1., *x + 2.), Rot(*x + 3., *x + 4., *x + 5.))) }).take(1000));
// for (i, e) in world.insert_from((Static,), data.clone()).iter().enumerate() {
//     let (pos, rot) = data.get(i).unwrap();
//     expected.insert(*e, (Some(*pos), Some(*rot), None, None, Some(Static)));
// }

// // static, model(1) | pos, rot
// let data = Vec::from_iter(std::iter::unfold(0f32, |x| {*x += 1.; Some((Pos(*x + 1., *x + 1., *x + 2.), Rot(*x + 3., *x + 4., *x + 5.))) }).take(1000));
// for (i, e) in world.insert_from((Static, Model(1)), data.clone()).iter().enumerate() {
//     let (pos, rot) = data.get(i).unwrap();
//     expected.insert(*e, (Some(*pos), Some(*rot), None, Some(Model(1)), Some(Static)));
// }

// // pos, rot, vel
// let data = Vec::from_iter(std::iter::unfold(0f32, |x| {
//     *x += 1.;
//     Some((Pos(*x + 1., *x + 1., *x + 2.), Rot(*x + 3., *x + 4., *x + 5.), Vel(*x + 6., *x + 7., *x + 8.)))
// }).take(1000));
// for (i, e) in world.insert_from((), data.clone()).iter().enumerate() {
//     let (pos, rot, vel) = data.get(i).unwrap();
//     expected.insert(*e, (Some(*pos), Some(*rot), Some(*vel), None, None));
// }

//     (world, expected)
// }

#[test]
fn query_read_entity_data() {
    let universe = Universe::new(None);
    let mut world = universe.create_world();

    let shared = (Static, Model(5));
    let components = vec![
        (Pos(1., 2., 3.), Rot(0.1, 0.2, 0.3)),
        (Pos(4., 5., 6.), Rot(0.4, 0.5, 0.6)),
    ];

    let mut expected = HashMap::<Entity, (Pos, Rot)>::new();

    for (i, e) in world
        .insert_from(shared, components.clone())
        .iter()
        .enumerate()
    {
        if let Some((pos, rot)) = components.get(i) {
            expected.insert(*e, (*pos, *rot));
        }
    }

    let query = Read::<Pos>::query(&world);

    let mut count = 0;
    for (entity, pos) in query.into_data_with_entities() {
        assert_eq!(&expected.get(&entity).unwrap().0, pos);
        count += 1;
    }

    assert_eq!(components.len(), count);
}

#[test]
fn query_read_entity_data_tuple() {
    let universe = Universe::new(None);
    let mut world = universe.create_world();

    let shared = (Static, Model(5));
    let components = vec![
        (Pos(1., 2., 3.), Rot(0.1, 0.2, 0.3)),
        (Pos(4., 5., 6.), Rot(0.4, 0.5, 0.6)),
    ];

    let mut expected = HashMap::<Entity, (Pos, Rot)>::new();

    for (i, e) in world
        .insert_from(shared, components.clone())
        .iter()
        .enumerate()
    {
        if let Some((pos, rot)) = components.get(i) {
            expected.insert(*e, (*pos, *rot));
        }
    }

    let query = <(Read<Pos>, Read<Rot>)>::query(&world);

    let mut count = 0;
    for (entity, (pos, rot)) in query.into_data_with_entities() {
        assert_eq!(&expected.get(&entity).unwrap().0, pos);
        assert_eq!(&expected.get(&entity).unwrap().1, rot);
        count += 1;
    }

    assert_eq!(components.len(), count);
}

#[test]
fn query_write_entity_data() {
    let universe = Universe::new(None);
    let mut world = universe.create_world();

    let shared = (Static, Model(5));
    let components = vec![
        (Pos(1., 2., 3.), Rot(0.1, 0.2, 0.3)),
        (Pos(4., 5., 6.), Rot(0.4, 0.5, 0.6)),
    ];

    let mut expected = HashMap::<Entity, (Pos, Rot)>::new();

    for (i, e) in world
        .insert_from(shared, components.clone())
        .iter()
        .enumerate()
    {
        if let Some((pos, rot)) = components.get(i) {
            expected.insert(*e, (*pos, *rot));
        }
    }

    let query = Write::<Pos>::query(&mut world);

    let mut count = 0;
    for (entity, pos) in query.into_data_with_entities() {
        assert_eq!(&expected.get(&entity).unwrap().0, pos);
        count += 1;

        pos.0 = 0.0;
    }

    assert_eq!(components.len(), count);
}

#[test]
fn query_write_entity_data_tuple() {
    let universe = Universe::new(None);
    let mut world = universe.create_world();

    let shared = (Static, Model(5));
    let components = vec![
        (Pos(1., 2., 3.), Rot(0.1, 0.2, 0.3)),
        (Pos(4., 5., 6.), Rot(0.4, 0.5, 0.6)),
    ];

    let mut expected = HashMap::<Entity, (Pos, Rot)>::new();

    for (i, e) in world
        .insert_from(shared, components.clone())
        .iter()
        .enumerate()
    {
        if let Some((pos, rot)) = components.get(i) {
            expected.insert(*e, (*pos, *rot));
        }
    }

    let query = <(Write<Pos>, Write<Rot>)>::query(&mut world);

    let mut count = 0;
    for (entity, (pos, rot)) in query.into_data_with_entities() {
        assert_eq!(&expected.get(&entity).unwrap().0, pos);
        assert_eq!(&expected.get(&entity).unwrap().1, rot);
        count += 1;

        pos.0 = 0.0;
        rot.0 = 0.0;
    }

    assert_eq!(components.len(), count);
}

#[test]
fn query_mixed_entity_data_tuple() {
    let universe = Universe::new(None);
    let mut world = universe.create_world();

    let shared = (Static, Model(5));
    let components = vec![
        (Pos(1., 2., 3.), Rot(0.1, 0.2, 0.3)),
        (Pos(4., 5., 6.), Rot(0.4, 0.5, 0.6)),
    ];

    let mut expected = HashMap::<Entity, (Pos, Rot)>::new();

    for (i, e) in world
        .insert_from(shared, components.clone())
        .iter()
        .enumerate()
    {
        if let Some((pos, rot)) = components.get(i) {
            expected.insert(*e, (*pos, *rot));
        }
    }

    let query = <(Read<Pos>, Write<Rot>)>::query(&mut world);

    let mut count = 0;
    for (entity, (pos, rot)) in query.into_data_with_entities() {
        assert_eq!(&expected.get(&entity).unwrap().0, pos);
        assert_eq!(&expected.get(&entity).unwrap().1, rot);
        count += 1;

        rot.0 = 0.0;
    }

    assert_eq!(components.len(), count);
}

#[test]
fn query_partial_match() {
    let universe = Universe::new(None);
    let mut world = universe.create_world();

    let shared = (Static, Model(5));
    let components = vec![
        (Pos(1., 2., 3.), Rot(0.1, 0.2, 0.3)),
        (Pos(4., 5., 6.), Rot(0.4, 0.5, 0.6)),
    ];

    let mut expected = HashMap::<Entity, (Pos, Rot)>::new();

    for (i, e) in world
        .insert_from(shared, components.clone())
        .iter()
        .enumerate()
    {
        if let Some((pos, rot)) = components.get(i) {
            expected.insert(*e, (*pos, *rot));
        }
    }

    let query = <(Read<Pos>, Write<Rot>)>::query(&mut world);

    let mut count = 0;
    for (entity, (pos, rot)) in query.into_data_with_entities() {
        assert_eq!(&expected.get(&entity).unwrap().0, pos);
        assert_eq!(&expected.get(&entity).unwrap().1, rot);
        count += 1;

        rot.0 = 0.0;
    }

    assert_eq!(components.len(), count);
}

#[test]
fn query_read_shared_data() {
    let universe = Universe::new(None);
    let mut world = universe.create_world();

    let shared = (Static, Model(5));
    let components = vec![
        (Pos(1., 2., 3.), Rot(0.1, 0.2, 0.3)),
        (Pos(4., 5., 6.), Rot(0.4, 0.5, 0.6)),
    ];

    world.insert_from(shared, components.clone());

    let query = Shared::<Static>::query(&world);

    let mut count = 0;
    for marker in query.into_data() {
        assert_eq!(&Static, marker);
        count += 1;
    }

    assert_eq!(components.len(), count);
}
