use std::collections::HashMap;

use dialga::EntityFabricator;
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

fn setup_world() -> (World, EntityFabricator) {
    let mut world = World::new();
    world.register_component::<Positioned>();
    world.register_component::<Named>();
    world.register_component::<PhysicBody>();
    world.register_component::<HasHP>();
    world.register_component::<FactionAffiliations>();

    let mut fab = EntityFabricator::new();
    fab.register::<Named>("name");
    fab.register::<PhysicBody>("physic-body");
    fab.register::<HasHP>("has-hp");
    fab.register::<FactionAffiliations>("factions");

    (world, fab)
}

#[test]
fn test() {
    let (mut world, mut fab) = setup_world();

    let bp_src = r#"
    grass {
        physic-body mass=10
        has-hp start-hp=10
    }

    cat {
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

    housecat inherit="cat" {
        name "Macy"
    }

    puma inherit="cat" {
        physic-body mass=150
        // We'll ignore HP and stuff
    }
    "#;
    fab.load_str(bp_src, "example.kdl")
        .unwrap_or_else(|e| panic!("{:?}", miette::Report::new(e)));

    let grass = fab
        .instantiate("grass", world.spawn())
        .unwrap_or_else(|(err, _)| panic!("{}", err))
        .build();
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

    let housecat = fab
        .instantiate("housecat", world.spawn())
        .unwrap_or_else(|(err, _)| panic!("{}", err))
        .build();
    {
        let (name, pb, hp, fa) = world
            .query::<(&Named, &PhysicBody, &HasHP, &FactionAffiliations)>(housecat)
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

    let puma = fab
        .instantiate("puma", world.spawn())
        .unwrap_or_else(|(err, _)| panic!("{}", err))
        .build();
    {
        let (pb, hp, fa) = world
            .query::<(&PhysicBody, &HasHP, &FactionAffiliations)>(puma)
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
