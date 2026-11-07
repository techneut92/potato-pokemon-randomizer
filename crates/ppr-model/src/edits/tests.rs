mod validate;

use super::*;
use crate::game::tests::data;
use crate::{
    EncounterKind, EncounterSet, Generation, InGameTrade, Item, Move, MoveCategory, Shop, StaticEncounter, Trainer,
    TrainerPokemon, WildEncounter,
};

/// A Gen 3 game with one object of every kind an edit can change.
fn game() -> GameData {
    let mut g = data(&[]);
    g.types = Type::in_generation(g.identity.generation);
    for s in &mut g.species {
        s.abilities = vec![1, 0];
        s.stats = crate::BaseStats {
            hp: 50,
            attack: 50,
            defense: 50,
            speed: 50,
            sp_attack: 50,
            sp_defense: 50,
        };
    }
    g.ability_names = ["-", "Overgrow", "Blaze", "Torrent"].map(String::from).to_vec();
    g.items = (1..=10)
        .map(|id| Item {
            id,
            name: format!("Item{id}"),
            allowed: true,
            bad: false,
            tm: false,
        })
        .collect();
    g.starter_held_items = vec![None];
    g.moves = (1..=4)
        .map(|n| Move::basic(n, "Move", 40, Type::Normal, MoveCategory::Physical))
        .collect();
    let mut first = TrainerPokemon::new(1, 5);
    first.ivs = 12;
    first.ability_slot = 2;
    g.trainers = vec![Trainer::new(7, "Joey", 1, vec![first, TrainerPokemon::new(2, 6)])];
    g.encounters = vec![EncounterSet {
        kind: EncounterKind::Walking,
        map: None,
        name: "ROUTE 1".into(),
        location: None,
        post_game: false,
        post_game_from: None,
        rate: 20,
        banned: Vec::new(),
        encounters: vec![
            WildEncounter {
                species: 1,
                forme: 0,
                min_level: 2,
                max_level: 3,
            };
            2
        ],
    }];
    let mut egg = StaticEncounter::new(4, 1);
    egg.egg = true;
    g.statics = vec![StaticEncounter::new(3, 30), egg];
    g.trades = vec![InGameTrade {
        requested: Some(1),
        given: 2,
        nickname: "BUDDY".into(),
        ot_name: "KATE".into(),
        ot_id: 1234,
        ivs: vec![5; 6],
        held_item: None,
    }];
    g.shops = vec![Shop {
        name: "Mart".into(),
        items: vec![1, 2],
        main_game: true,
        special: false,
    }];
    g.item_prices = [(1, 200), (2, 300)].into();
    g.field_items = vec![3, 4];
    g
}

fn stats(each: u8) -> StatsEdit {
    StatsEdit {
        hp: each,
        attack: each,
        defense: each,
        speed: each,
        sp_attack: each,
        sp_defense: each,
    }
}

/// One valid edit of every kind for [`game`].
fn every_kind() -> Vec<Edit> {
    vec![
        Edit::Starter(StarterEdit { slot: 2, species: 3 }),
        Edit::StarterHeldItem(StarterHeldItemEdit { slot: 0, item: Some(5) }),
        Edit::Species(SpeciesEdit {
            number: 4,
            primary_type: Type::Fire,
            secondary_type: Some(Type::Dragon),
            stats: stats(90),
            abilities: vec![2, 3],
        }),
        Edit::Trainer(TrainerEdit {
            index: 7,
            pokemon: vec![
                TrainerPokemonEdit {
                    species: 6,
                    level: 50,
                    held_item: Some(9),
                    moves: Some([1, 2, 0, 0]),
                },
                TrainerPokemonEdit {
                    species: 5,
                    level: 51,
                    held_item: None,
                    moves: None,
                },
                TrainerPokemonEdit {
                    species: 4,
                    level: 52,
                    held_item: None,
                    moves: None,
                },
            ],
        }),
        Edit::WildSet(WildSetEdit {
            index: 0,
            slots: vec![
                WildSlotEdit {
                    species: 5,
                    min_level: 10,
                    max_level: 12,
                },
                WildSlotEdit {
                    species: 6,
                    min_level: 100,
                    max_level: 100,
                },
            ],
        }),
        Edit::Static(StaticEdit {
            index: 0,
            species: 6,
            level: 70,
        }),
        Edit::Trade(TradeEdit {
            index: 0,
            requested: None,
            given: 5,
            nickname: "PAL".into(),
            ot_name: "ASH".into(),
            ot_id: 99,
            ivs: vec![31, 0, 31, 0, 31, 0],
            held_item: Some(8),
        }),
        Edit::Shop(ShopEdit {
            index: 0,
            items: vec![10, 9, 8],
        }),
        Edit::ItemPrice(ItemPriceEdit { item: 2, price: 9999 }),
        Edit::FieldItem(FieldItemEdit { index: 1, item: 7 }),
    ]
}

fn list(g: &GameData, edits: Vec<Edit>) -> EditList {
    EditList {
        edits,
        ..EditList::new(&g.identity)
    }
}

/// The errors applying `edit` alone to [`game`] gives; the game must stay unchanged.
fn errors(edit: Edit) -> Vec<EditError> {
    let mut g = game();
    let result = list(&g, vec![edit]).apply(&mut g);
    assert_eq!(g, game(), "a failed apply leaves the game as it was");
    result.expect_err("the edit doesn't fit")
}

#[test]
fn edit_lists_round_trip_through_json() {
    let g = game();
    let edits = list(&g, every_kind());
    let json = serde_json::to_string(&edits).expect("serializes");
    let back: EditList = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(back, edits);
}

#[test]
fn the_json_names_kinds_and_fields_in_camel_case() {
    let edit = Edit::WildSet(WildSetEdit {
        index: 3,
        slots: vec![WildSlotEdit {
            species: 1,
            min_level: 2,
            max_level: 3,
        }],
    });
    assert_eq!(
        serde_json::to_value(&edit).expect("serializes"),
        serde_json::json!({ "kind": "wildSet", "index": 3, "slots": [{ "species": 1, "minLevel": 2, "maxLevel": 3 }] })
    );
    let target = serde_json::to_value(ObjectRef::ItemPrice { item: 4 }).expect("serializes");
    assert_eq!(target, serde_json::json!({ "kind": "itemPrice", "item": 4 }));
}

#[test]
fn every_kind_of_edit_is_applied() {
    let mut g = game();
    list(&g, every_kind()).apply(&mut g).expect("every edit fits");

    assert_eq!(g.starters, [1, 4, 3]);
    assert_eq!(g.starter_held_items, [Some(5)]);

    let sp = g.species(4).expect("species 4");
    assert_eq!((sp.primary_type, sp.secondary_type), (Type::Fire, Some(Type::Dragon)));
    assert_eq!(sp.stats.total(), 540);
    assert_eq!(sp.abilities, [2, 3]);
    assert_eq!(
        g.original_types_of(4),
        Some((Type::Normal, None)),
        "the types as loaded stay known"
    );

    let team = &g.trainers[0].pokemon;
    assert_eq!(team.len(), 3);
    assert_eq!((team[0].species, team[0].level, team[0].held_item), (6, 50, Some(9)));
    assert_eq!((team[0].moves, team[0].reset_moves), ([1, 2, 0, 0], false));
    assert_eq!(
        (team[0].ivs, team[0].ability_slot, team[0].added),
        (12, 2, false),
        "what the edit doesn't set stays"
    );
    assert_eq!((team[1].moves, team[1].reset_moves), ([0; 4], true));
    assert_eq!(
        (team[2].species, team[2].ivs, team[2].ability_slot, team[2].added),
        (4, 0, 1, true),
        "an added Pokémon gets the last one's IVs"
    );

    let slots = &g.encounters[0].encounters;
    assert_eq!(
        slots
            .iter()
            .map(|s| (s.species, s.min_level, s.max_level))
            .collect::<Vec<_>>(),
        [(5, 10, 12), (6, 100, 100)]
    );
    assert_eq!((g.statics[0].species, g.statics[0].level), (6, 70));

    let t = &g.trades[0];
    assert_eq!((t.requested, t.given, t.nickname.as_str()), (None, 5, "PAL"));
    assert_eq!((t.ot_name.as_str(), t.ot_id, t.held_item), ("ASH", 99, Some(8)));
    assert_eq!(t.ivs, [31, 0, 31, 0, 31, 0]);

    assert_eq!(g.shops[0].items, [10, 9, 8]);
    assert_eq!(g.item_prices[&2], 9999);
    assert_eq!(g.field_items, [3, 7]);
}

#[test]
fn an_empty_list_changes_nothing() {
    let mut g = game();
    EditList::new(&g.identity).apply(&mut g).expect("nothing to apply");
    assert_eq!(g, game());
}

#[test]
fn set_replaces_and_revert_removes_the_edit_of_an_object() {
    let g = game();
    let mut edits = EditList::new(&g.identity);
    edits.set(Edit::FieldItem(FieldItemEdit { index: 0, item: 1 }));
    edits.set(Edit::FieldItem(FieldItemEdit { index: 1, item: 2 }));
    edits.set(Edit::FieldItem(FieldItemEdit { index: 0, item: 5 }));
    assert_eq!(
        edits.edits,
        [
            Edit::FieldItem(FieldItemEdit { index: 0, item: 5 }),
            Edit::FieldItem(FieldItemEdit { index: 1, item: 2 })
        ]
    );
    assert!(edits.revert(&ObjectRef::FieldItem { index: 0 }));
    assert!(!edits.revert(&ObjectRef::FieldItem { index: 0 }));
    assert_eq!(edits.edits.len(), 1);
}

#[test]
fn a_list_for_another_game_or_format_is_rejected() {
    let mut g = game();
    let mut other = g.identity.clone();
    other.name = "Ruby (U)".into();
    other.code = "AXVE".into();
    let foreign = EditList::new(&other);
    assert_eq!(
        foreign.apply(&mut g),
        Err(vec![EditError::WrongGame {
            expected: "Test".into(),
            found: "Ruby (U)".into()
        }])
    );

    let mut revision = g.identity.clone();
    revision.version = Some(1);
    assert!(EditList::new(&revision).check(&g.identity).is_err(), "another revision");

    let future = EditList {
        version: EDIT_LIST_VERSION + 1,
        ..EditList::new(&g.identity)
    };
    assert_eq!(
        future.check(&g.identity),
        Err(EditError::UnsupportedVersion {
            found: EDIT_LIST_VERSION + 1
        })
    );
}

#[test]
fn unknown_objects_are_rejected() {
    for edit in [
        Edit::Starter(StarterEdit { slot: 3, species: 1 }),
        Edit::StarterHeldItem(StarterHeldItemEdit { slot: 1, item: None }),
        Edit::Species(SpeciesEdit {
            number: 7,
            primary_type: Type::Normal,
            secondary_type: None,
            stats: stats(50),
            abilities: vec![1, 0],
        }),
        Edit::Trainer(TrainerEdit {
            index: 8,
            pokemon: Vec::new(),
        }),
        Edit::WildSet(WildSetEdit {
            index: 1,
            slots: Vec::new(),
        }),
        Edit::Static(StaticEdit {
            index: 2,
            species: 1,
            level: 5,
        }),
        trade_at(1),
        Edit::Shop(ShopEdit {
            index: 1,
            items: vec![1],
        }),
        Edit::ItemPrice(ItemPriceEdit { item: 3, price: 1 }),
        Edit::FieldItem(FieldItemEdit { index: 2, item: 1 }),
    ] {
        let target = edit.target();
        assert_eq!(errors(edit), [EditError::UnknownObject { target }], "{target}");
    }
}

#[test]
fn every_error_is_reported_and_nothing_is_applied() {
    let mut g = game();
    let mut edits = every_kind();
    edits.push(Edit::Starter(StarterEdit { slot: 0, species: 99 }));
    edits.push(Edit::FieldItem(FieldItemEdit { index: 0, item: 77 }));
    edits.push(Edit::FieldItem(FieldItemEdit { index: 0, item: 1 }));
    let result = list(&g, edits).apply(&mut g);
    assert_eq!(g, game());
    let target = ObjectRef::FieldItem { index: 0 };
    assert_eq!(
        result,
        Err(vec![
            EditError::UnknownSpecies {
                target: ObjectRef::Starter { slot: 0 },
                species: 99
            },
            EditError::UnknownItem { target, item: 77 },
            EditError::Duplicate { target },
        ])
    );
}

fn trainer(pokemon: Vec<TrainerPokemonEdit>) -> Edit {
    Edit::Trainer(TrainerEdit { index: 7, pokemon })
}

fn member(level: u8, moves: Option<[MoveId; 4]>) -> TrainerPokemonEdit {
    TrainerPokemonEdit {
        species: 1,
        level,
        held_item: None,
        moves,
    }
}

#[test]
fn levels_and_team_sizes_are_checked() {
    let target = ObjectRef::Trainer { index: 7 };
    let range = |field, value, min, max| EditError::OutOfRange {
        target,
        field,
        value,
        min,
        max,
    };
    assert_eq!(errors(trainer(Vec::new())), [range("team size", 0, 1, 6)]);
    assert_eq!(errors(trainer(vec![member(5, None); 7])), [range("team size", 7, 1, 6)]);
    assert_eq!(
        errors(trainer(vec![member(0, None), member(101, None)])),
        [range("level", 0, 1, 100), range("level", 101, 1, 100)]
    );

    let wild = |min_level, max_level| {
        Edit::WildSet(WildSetEdit {
            index: 0,
            slots: vec![
                WildSlotEdit {
                    species: 1,
                    min_level,
                    max_level,
                };
                2
            ],
        })
    };
    let target = ObjectRef::WildSet { index: 0 };
    assert_eq!(
        errors(wild(9, 8)),
        vec![
            EditError::Invalid {
                target,
                reason: "the lowest level is above the highest"
            };
            2
        ]
    );
    assert_eq!(
        errors(Edit::WildSet(WildSetEdit {
            index: 0,
            slots: Vec::new()
        })),
        [EditError::WrongCount {
            target,
            field: "slots",
            expected: 2,
            found: 0
        }]
    );
}

#[test]
fn moves_items_and_species_must_exist() {
    let target = ObjectRef::Trainer { index: 7 };
    assert_eq!(
        errors(trainer(vec![member(5, Some([1, 5, 0, 0]))])),
        [EditError::UnknownMove { target, mv: 5 }]
    );
    assert!(matches!(
        errors(trainer(vec![member(5, Some([0; 4]))]))[..],
        [EditError::Invalid { .. }]
    ));
    let mut holder = member(5, None);
    holder.held_item = Some(11);
    holder.species = 0;
    assert_eq!(
        errors(trainer(vec![holder])),
        [
            EditError::UnknownSpecies { target, species: 0 },
            EditError::UnknownItem { target, item: 11 }
        ]
    );
    let target = ObjectRef::Shop { index: 0 };
    assert_eq!(
        errors(Edit::Shop(ShopEdit {
            index: 0,
            items: Vec::new()
        })),
        [EditError::Invalid {
            target,
            reason: "a shop must sell at least one item"
        }]
    );
}

#[test]
fn species_types_stats_and_abilities_are_checked() {
    let target = ObjectRef::Species { number: 1 };
    let edit = |primary_type, secondary_type, stats, abilities| {
        Edit::Species(SpeciesEdit {
            number: 1,
            primary_type,
            secondary_type,
            stats,
            abilities,
        })
    };
    assert!(matches!(
        errors(edit(Type::Fire, Some(Type::Fire), stats(1), vec![1, 0]))[..],
        [EditError::Invalid { .. }]
    ));
    assert_eq!(
        errors(edit(Type::Fairy, None, stats(1), vec![1, 0])),
        [EditError::UnknownType {
            target,
            ty: Type::Fairy
        }],
        "no Fairy type in Gen 3"
    );
    assert_eq!(errors(edit(Type::Fire, None, stats(0), vec![1, 0])).len(), 6);
    assert_eq!(
        errors(edit(Type::Fire, None, stats(1), vec![1])),
        [EditError::WrongCount {
            target,
            field: "abilities",
            expected: 2,
            found: 1
        }]
    );
    assert_eq!(
        errors(edit(Type::Fire, None, stats(1), vec![1, 4])),
        [EditError::UnknownAbility { target, ability: 4 }]
    );
    assert!(matches!(
        errors(edit(Type::Fire, None, stats(1), vec![0, 1]))[..],
        [EditError::Invalid { .. }]
    ));
}

/// [`game`] as a Gen 1 or Gen 2 game: no abilities, and Gen 2's four DVs for trades.
fn old_game(generation: u8) -> GameData {
    let mut g = game();
    g.identity.generation = Generation::new(generation).expect("valid");
    g.types = Type::in_generation(g.identity.generation);
    for s in &mut g.species {
        s.abilities.clear();
    }
    g.ability_names.clear();
    let t = &mut g.trades[0];
    if generation == 1 {
        (t.ot_name, t.ot_id, t.ivs) = (String::new(), 0, Vec::new());
    } else {
        t.ivs = vec![5; 4];
    }
    g
}

fn old_errors(generation: u8, edit: Edit) -> Vec<EditError> {
    let mut g = old_game(generation);
    list(&g, vec![edit]).apply(&mut g).expect_err("the edit doesn't fit")
}

fn trade_at(index: usize) -> Edit {
    match trade("ASH", vec![0; 6], None) {
        Edit::Trade(t) => Edit::Trade(TradeEdit { index, ..t }),
        other => other,
    }
}

fn trade(ot_name: &str, ivs: Vec<u8>, held_item: Option<ItemId>) -> Edit {
    Edit::Trade(TradeEdit {
        index: 0,
        requested: Some(1),
        given: 2,
        nickname: "BUDDY".into(),
        ot_name: ot_name.into(),
        ot_id: 0,
        ivs,
        held_item,
    })
}

#[test]
fn fields_a_generation_lacks_are_rejected() {
    let target = ObjectRef::Trade { index: 0 };
    let unsupported = |field| EditError::Unsupported { target, field };
    assert_eq!(
        old_errors(1, trade("ASH", vec![1; 4], Some(1))),
        [
            unsupported("original trainer"),
            unsupported("IVs"),
            unsupported("held items")
        ]
    );
    let mut g = old_game(1);
    list(&g, vec![trade("", Vec::new(), None)])
        .apply(&mut g)
        .expect("a Gen 1 trade without them fits");

    let held = trainer(vec![TrainerPokemonEdit {
        held_item: Some(1),
        ..member(5, None)
    }]);
    assert_eq!(
        old_errors(1, held),
        [EditError::Unsupported {
            target: ObjectRef::Trainer { index: 7 },
            field: "held items"
        }]
    );

    let species = Edit::Species(SpeciesEdit {
        number: 1,
        primary_type: Type::Normal,
        secondary_type: None,
        stats: stats(50),
        abilities: vec![1, 0],
    });
    assert_eq!(
        old_errors(2, species),
        [EditError::Unsupported {
            target: ObjectRef::Species { number: 1 },
            field: "abilities"
        }]
    );

    assert_eq!(
        old_errors(2, trade("ASH", vec![16, 15, 0, 0], None)),
        [EditError::OutOfRange {
            target,
            field: "IV",
            value: 16,
            min: 0,
            max: 15
        }],
        "Gen 2 DVs are 0–15"
    );
    assert_eq!(
        errors(trade("ASH", vec![32, 31, 0, 0, 0, 0], None)),
        [EditError::OutOfRange {
            target,
            field: "IV",
            value: 32,
            min: 0,
            max: 31
        }]
    );
}

#[test]
fn trainer_ids_fit_what_the_generation_stores() {
    let with_id = |ot_id| match trade("ASH", vec![0; 6], None) {
        Edit::Trade(t) => Edit::Trade(TradeEdit { ot_id, ..t }),
        other => other,
    };
    assert_eq!(
        errors(with_id(65_536)),
        [EditError::OutOfRange {
            target: ObjectRef::Trade { index: 0 },
            field: "original trainer ID",
            value: 65_536,
            min: 0,
            max: 65_535
        }],
        "Gen 3 to 6 store 16 bits"
    );
    let mut g = game();
    list(&g, vec![with_id(65_535)]).apply(&mut g).expect("16 bits fit");

    let mut g = game();
    g.identity.generation = Generation::new(7).expect("valid");
    list(&g, vec![with_id(3_000_000_000)])
        .apply(&mut g)
        .expect("Gen 7 stores 32 bits");
    assert_eq!(g.trades[0].ot_id, 3_000_000_000);
}

#[test]
fn an_egg_keeps_its_level() {
    let egg = |level| {
        Edit::Static(StaticEdit {
            index: 1,
            species: 5,
            level,
        })
    };
    assert_eq!(
        errors(egg(20)),
        [EditError::Unsupported {
            target: ObjectRef::Static { index: 1 },
            field: "level for an egg"
        }]
    );
    let mut g = game();
    list(&g, vec![egg(1)]).apply(&mut g).expect("same level");
    assert_eq!((g.statics[1].species, g.statics[1].level), (5, 1));
}

#[test]
fn errors_name_the_object() {
    let e = EditError::OutOfRange {
        target: ObjectRef::Trainer { index: 7 },
        field: "level",
        value: 101,
        min: 1,
        max: 100,
    };
    assert_eq!(e.to_string(), "trainer 7: level is 101, but must be 1–100");
    assert_eq!(
        EditError::UnknownObject {
            target: ObjectRef::Starter { slot: 3 }
        }
        .to_string(),
        "starter 4: it is not in this game"
    );
}
