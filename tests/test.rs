use std::collections::HashMap;

use dialga::{blueprint::BlueprintLookupError, EntityFabricator, InstantiationError};
use palkia::prelude::*;
use serde::Deserialize;

macro_rules! impl_component {
    ($ty:ty) => {
        impl Component for $ty {
            fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
            where
                Self: Sized,
            {
                builder
            }
        }
    };
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct TrackedPosition;
impl_component!(TrackedPosition);

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Positioned {
    x: i32,
    y: i32,
}
impl_component!(Positioned);

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Named(String);
impl_component!(Named);

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct PhysicBody {
    mass: u32,
}
impl_component!(PhysicBody);

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct HasHP {
    start_hp: u32,
    #[serde(default)]
    resistances: HashMap<String, i32>,
}
impl_component!(HasHP);

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct FactionAffiliations {
    member_of: String,
    liked_by: Vec<String>,
    disliked_by: Vec<String>,
}
impl_component!(FactionAffiliations);

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Legendary;
impl_component!(Legendary);

fn setup_world() -> World {
    let mut world = World::new();
    world.register_component::<TrackedPosition>();
    world.register_component::<Positioned>();
    world.register_component::<Named>();
    world.register_component::<PhysicBody>();
    world.register_component::<HasHP>();
    world.register_component::<FactionAffiliations>();
    world.register_component::<Legendary>();
    world
}

fn setup_fab() -> EntityFabricator {
    let mut fab = EntityFabricator::new();
    fab.register::<TrackedPosition>("tracked-position");
    fab.register::<Named>("name");
    fab.register::<PhysicBody>("physic-body");
    fab.register::<HasHP>("has-hp");
    fab.register::<FactionAffiliations>("factions");
    fab.register::<Legendary>("legendary");
    fab
}

fn setup_both() -> (World, EntityFabricator) {
    (setup_world(), setup_fab())
}

#[test]
fn test() {
    let (mut world, mut fab) = setup_both();

    let bp_src = r#"
    // Two example splicees -- in real life these will be more complicated
    mob {
        tracked-position
    }

    legend {
        legendary
    }

    grass {
        physic-body mass=10
        has-hp start-hp=10
    }

    cat {
        (splice)mob
        physic-body mass=50
        has-hp { 
            start-hp 10
            resistances falling=100 ice=-20
        }
        factions {
            member-of "cats"
            liked-by "humans" "elves"
            disliked-by "dogs" "dwarves"
        }
    }

    housecat {
        (splice)cat
        name "Macy"
    }

    puma {
        (splice)cat
        physic-body mass=150
        (splice)legend
    }
    "#;
    fab.load_str(bp_src, "example.kdl")
        .unwrap_or_else(|e| panic!("{:?}", miette::Report::new(e)));

    let grass = fab.instantiate("grass", world.spawn()).unwrap().build();
    {
        let (pb, hp) = world.query::<(&PhysicBody, &HasHP)>(grass).unwrap();
        assert_eq!(*pb, PhysicBody { mass: 10 },);
        assert_eq!(
            *hp,
            HasHP {
                start_hp: 10,
                resistances: HashMap::new(),
            },
        );
    }

    let housecat = fab.instantiate("housecat", world.spawn()).unwrap().build();
    {
        let (name, pb, hp, fa, _tp) = world
            .query::<(
                &Named,
                &PhysicBody,
                &HasHP,
                &FactionAffiliations,
                &TrackedPosition,
            )>(housecat)
            .unwrap();

        assert_eq!(*name, Named("Macy".to_owned()));
        assert_eq!(*pb, PhysicBody { mass: 50 });
        assert_eq!(
            *hp,
            HasHP {
                start_hp: 10,
                resistances: [("falling".to_string(), 100), ("ice".to_string(), -20)]
                    .into_iter()
                    .collect(),
            }
        );

        assert_eq!(
            *fa,
            FactionAffiliations {
                member_of: "cats".to_string(),
                liked_by: vec!["humans".to_owned(), "elves".to_owned()],
                disliked_by: vec!["dogs".to_owned(), "dwarves".to_owned()],
            }
        );
    }

    let puma = fab.instantiate("puma", world.spawn()).unwrap().build();
    {
        let (pb, hp, fa, _tp, _leg) = world
            .query::<(
                &PhysicBody,
                &HasHP,
                &FactionAffiliations,
                &TrackedPosition,
                &Legendary,
            )>(puma)
            .unwrap();

        assert_eq!(*pb, PhysicBody { mass: 150 });
        assert_eq!(
            *hp,
            HasHP {
                start_hp: 10,
                resistances: [("falling".to_string(), 100), ("ice".to_string(), -20)]
                    .into_iter()
                    .collect(),
            }
        );
        assert_eq!(
            *fa,
            FactionAffiliations {
                member_of: "cats".to_string(),
                liked_by: vec!["humans".to_owned(), "elves".to_owned()],
                disliked_by: vec!["dogs".to_owned(), "dwarves".to_owned()],
            }
        );
    }
}

#[test]
fn error_unknown() {
    let mut world = setup_world();

    let bp_src = r#"
    oh-no {
        tracked-position
        erroring-comp
    }
    "#;
    let mut fab = setup_fab();
    fab.load_str(bp_src, "example.kdl")
        .unwrap_or_else(|e| panic!("{:?}", miette::Report::new(e)));

    let err = match fab.instantiate("unknown", world.spawn()) {
        Ok(_) => {
            panic!("expected error")
        }
        Err(it) => it,
    };
    assert_eq!(
        err,
        InstantiationError::BlueprintLookupError(BlueprintLookupError::BlueprintNotFound(
            "unknown".into()
        ))
    );

    let err = match fab.instantiate("oh-no", world.spawn()) {
        Ok(_) => {
            panic!("expected error")
        }
        Err(it) => it,
    };
    assert_eq!(err, InstantiationError::NoComponent("erroring-comp".into()));

    let bp_src = r#"
    alpha {
        physic-body mass=10
        (splice)beta
    }
    beta {
        has-hp start-hp=10
        (splice)gamma
    }
    gamma {
        legendary
        (splice)delta
    }
    delta {
        tracks-position
        (splice)alpha
    }
    entrypoint {
        (splice)alpha
    }

    failure {
        (splice)unknown
    }
    "#;
    let mut fab = setup_fab();
    fab.load_str(bp_src, "example.kdl")
        .unwrap_or_else(|e| panic!("{:?}", miette::Report::new(e)));

    let err = match fab.instantiate("failure", world.spawn()) {
        Ok(_) => {
            panic!("expected error")
        }
        Err(it) => it,
    };
    assert_eq!(
        err,
        InstantiationError::BlueprintLookupError(BlueprintLookupError::InheriteeNotFound(
            "failure".into(),
            "unknown".into()
        ))
    );

    let err = match fab.instantiate("entrypoint", world.spawn()) {
        Ok(_) => {
            panic!("expected error")
        }
        Err(it) => it,
    };
    assert_eq!(
        err,
        InstantiationError::BlueprintLookupError(BlueprintLookupError::InheritanceLoop(vec![
            "alpha".into(),
            "beta".into(),
            "gamma".into(),
            "delta".into(),
            "alpha".into(),
        ]))
    );
}
