//! The rules a handler reports and the validation of an edit list against them.

use super::*;
use crate::MultiBattleStatus;

fn rules(g: &GameData) -> EditRules {
    EditRules::from_game(g)
}

/// The issues of `edits` in `g` with `rules`.
fn issues_with(g: &GameData, rules: &EditRules, edits: Vec<Edit>) -> Vec<EditIssue> {
    list(g, edits).validate(g, rules).expect("the list is for this game")
}

fn issues(g: &GameData, edits: Vec<Edit>) -> Vec<EditIssue> {
    issues_with(g, &rules(g), edits)
}

fn messages(issues: &[EditIssue]) -> Vec<String> {
    issues.iter().map(|i| i.message.clone()).collect()
}

fn shop(items: Vec<ItemId>) -> Edit {
    Edit::Shop(ShopEdit { index: 0, items })
}

fn wild(slots: &[(u8, u8)]) -> Edit {
    Edit::WildSet(WildSetEdit {
        index: 0,
        slots: slots
            .iter()
            .map(|&(min_level, max_level)| WildSlotEdit {
                species: 1,
                min_level,
                max_level,
            })
            .collect(),
    })
}

fn team(size: usize) -> Edit {
    trainer((0..size).map(|_| member(10, None)).collect())
}

#[test]
fn rules_from_the_game_data() {
    let g = game();
    let r = rules(&g);
    assert_eq!(r.shops.size, ShopSize::Grow { max: None });
    assert_eq!(r.wild.levels, [WildLevels::Range]);
    assert_eq!(r.prices, PriceRules::of(g.identity.generation));
    assert_eq!(
        r.trades,
        TradeRules {
            texts: true,
            max_nickname_length: g.max_trade_nickname_length,
            max_ot_name_length: g.max_trade_ot_name_length,
            original_trainer: true,
            max_ot_id: 65_535,
            held_item: true,
            ivs: 6,
            max_iv: 31,
            random_iv: None,
        }
    );
    let joey = r.trainer(7).expect("listed");
    assert_eq!((joey.min_pokemon, joey.max_pokemon), (1, 6));
    assert!(joey.levels && joey.held_items && joey.custom_moves);

    let old = old_game(1);
    let r = rules(&old);
    assert_eq!(r.wild.levels, [WildLevels::Single]);
    assert!(!r.trades.original_trainer && !r.trades.held_item);
    assert_eq!(r.trades.max_ot_id, 0);
    assert!(!r.trainers.held_items && !r.trainers.custom_moves);
    assert_eq!(rules(&old_game(2)).trades.max_iv, 15);
    let mut gen7 = game();
    gen7.identity.generation = Generation::new(7).expect("valid");
    assert_eq!(
        rules(&gen7).trades.max_ot_id,
        u32::MAX,
        "Gen 7 stores 32-bit trainer IDs"
    );
}

#[test]
fn a_valid_list_has_no_issues() {
    let g = game();
    // The game's trade asks for a species, so the edit must too.
    let edits = every_kind()
        .into_iter()
        .map(|e| match e {
            Edit::Trade(t) => Edit::Trade(TradeEdit {
                requested: Some(2),
                ..t
            }),
            other => other,
        })
        .collect();
    assert_eq!(issues(&g, edits), []);
}

#[test]
fn a_list_for_another_game_fails_as_a_whole() {
    let g = game();
    let mut l = list(&g, every_kind());
    l.game.name = "Other".into();
    assert!(matches!(l.validate(&g, &rules(&g)), Err(EditError::WrongGame { .. })));
}

#[test]
fn errors_of_apply_become_issues_naming_the_object() {
    let g = game();
    let found = issues(&g, vec![team(7), shop(vec![99])]);
    assert_eq!(
        messages(&found),
        [
            "Trainer 7 (Joey): team size is 7, but must be 1–6",
            "Shop 1 (Mart): item 99 is not in this game",
        ]
    );
    assert_eq!(found[0].target, Some(ObjectRef::Trainer { index: 7 }));
    assert_eq!(found[0].field.as_deref(), Some("team size"));
    assert!(found.iter().all(|i| i.severity == EditSeverity::Error));
    let twice = issues(&g, vec![shop(vec![1]), shop(vec![2])]);
    assert_eq!(messages(&twice), ["Shop 1 (Mart): it is edited twice"]);
}

#[test]
fn whole_list_errors_become_issues_without_a_target() {
    let g = game();
    let issue = EditIssue::from_error(&EditError::UnsupportedVersion { found: 9 }, &g);
    assert_eq!(issue.target, None);
    assert!(issue.message.starts_with("These edits are in format 9"));
}

#[test]
fn fixed_shops_keep_their_size() {
    let g = game();
    let mut r = rules(&g);
    r.shops.size = ShopSize::Fixed;
    assert_eq!(issues_with(&g, &r, vec![shop(vec![3, 4])]), []);
    assert_eq!(
        messages(&issues_with(&g, &r, vec![shop(vec![1, 2, 3])])),
        ["Shop 1 (Mart): 3 items, but this game keeps every shop at its size, 2: remove 1"]
    );
    assert_eq!(
        messages(&issues_with(&g, &r, vec![shop(vec![1])])),
        ["Shop 1 (Mart): 1 items, but this game keeps every shop at its size, 2: add 1"]
    );
}

#[test]
fn shops_without_room_only_shrink() {
    let g = game();
    let mut r = rules(&g);
    r.shops.size = ShopSize::Shrink;
    assert_eq!(issues_with(&g, &r, vec![shop(vec![1])]), []);
    assert_eq!(issues_with(&g, &r, vec![shop(vec![1, 2, 3])]).len(), 1);
}

#[test]
fn growing_shops_have_a_most() {
    let g = game();
    let mut r = rules(&g);
    r.shops.size = ShopSize::Grow { max: Some(3) };
    assert_eq!(issues_with(&g, &r, vec![shop(vec![1, 2, 3])]), []);
    assert_eq!(
        messages(&issues_with(&g, &r, vec![shop(vec![1, 2, 3, 4])])),
        ["Shop 1 (Mart): 4 items, but this game's shops hold at most 3"]
    );
    r.shops.size = ShopSize::Grow { max: None };
    assert_eq!(issues_with(&g, &r, vec![shop((1..=10).collect())]), []);
}

#[test]
fn shared_shop_space_counts_every_shop() {
    let mut g = game();
    g.shops.push(Shop {
        name: "Other".into(),
        items: vec![3],
        main_game: true,
        special: true,
    });
    let mut r = rules(&g);
    // 4 × 2 + (2 + 2 × 1) = 12 bytes as loaded, and 4 to spare.
    r.shops.size = ShopSize::Shared {
        bytes: 16,
        costs: vec![ShopCost { base: 0, per_item: 4 }, ShopCost { base: 2, per_item: 2 }],
        only_changed: false,
        max: 255,
    };
    assert_eq!(issues_with(&g, &r, vec![shop(vec![1, 2, 3])]), []);
    let over = issues_with(&g, &r, vec![shop(vec![1, 2, 3, 4])]);
    assert_eq!(
        messages(&over),
        ["The shop lists need 4 bytes more than the ROM has room for. Remove items from any shop."]
    );
    assert_eq!(over[0].target, None);
    // Taking an item from the other shop makes room again.
    let other = Edit::Shop(ShopEdit {
        index: 1,
        items: vec![],
    });
    assert!(
        issues_with(&g, &r, vec![shop(vec![1, 2, 3, 4]), other]).len() > 1,
        "an empty shop is an error"
    );
    let other = Edit::Shop(ShopEdit {
        index: 1,
        items: vec![5],
    });
    assert_eq!(issues_with(&g, &r, vec![shop(vec![1, 2, 3, 4]), other]).len(), 1);
}

#[test]
fn shared_shop_space_of_changed_shops_only() {
    let g = game();
    let mut r = rules(&g);
    r.shops.size = ShopSize::Shared {
        bytes: 6,
        costs: vec![ShopCost { base: 0, per_item: 2 }],
        only_changed: true,
        max: 255,
    };
    assert_eq!(issues_with(&g, &r, vec![shop(vec![1, 2, 3])]), []);
    assert_eq!(issues_with(&g, &r, vec![shop(vec![1, 2, 3, 4])]).len(), 1);
    // An unchanged shop takes nothing.
    r.shops.size = ShopSize::Shared {
        bytes: 0,
        costs: vec![ShopCost { base: 0, per_item: 2 }],
        only_changed: true,
        max: 255,
    };
    assert_eq!(issues_with(&g, &r, vec![shop(vec![1, 2])]), []);
}

#[test]
fn prices_follow_the_generation() {
    let price = |item, price| Edit::ItemPrice(ItemPriceEdit { item, price });
    let mut g = game();
    assert_eq!(issues(&g, vec![price(1, 65_535)]), []);
    g.identity.generation = Generation::new(5).expect("valid");
    assert_eq!(
        messages(&issues(&g, vec![price(1, 1_235)])),
        ["Price of Item1: 1235, but this game stores prices in steps of 10"]
    );
    g.identity.generation = Generation::new(1).expect("valid");
    g.items[0].tm = true;
    assert_eq!(
        messages(&issues(&g, vec![price(1, 2_500)])),
        ["Price of Item1: 2500, but this game stores prices in steps of 1000"]
    );
}

#[test]
fn a_team_that_can_not_grow_keeps_its_size() {
    let mut g = game();
    g.trainer_pokemon_additions = [true, true, false];
    let r = rules(&g);
    assert_eq!(r.trainer(7).map(|t| t.max_pokemon), Some(2));
    assert_eq!(issues_with(&g, &r, vec![team(1)]), []);
    assert_eq!(
        messages(&issues_with(&g, &r, vec![team(3)])),
        ["Trainer 7 (Joey): 3 Pokémon, but this trainer's team can't grow beyond 2"]
    );
}

#[test]
fn battle_styles_and_shared_battles_limit_teams() {
    let mut g = game();
    g.trainers[0].battle_style = crate::BattleStyle::Triple;
    g.trainers[0].multi_battle = MultiBattleStatus::Potential;
    let found = issues(&g, vec![team(1)]);
    assert_eq!(
        messages(&found),
        ["Trainer 7 (Joey): 1 Pokémon, but its battle style needs at least 2"]
    );
    // A double battle of the game itself with one Pokémon may keep one.
    g.trainers[0].battle_style = crate::BattleStyle::Double;
    g.trainers[0].pokemon.truncate(1);
    assert_eq!(issues(&g, vec![team(1)]), []);
    assert_eq!(rules(&g).trainer(7).map(|t| t.min_pokemon), Some(1));
    let found = issues(&g, vec![team(4)]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, EditSeverity::Warning);
    g.trainers[0].multi_battle = MultiBattleStatus::Always;
    assert_eq!(issues(&g, vec![team(4)])[0].severity, EditSeverity::Error);
    // A team the game itself has bigger than 3 may keep its size, with no warning.
    g.trainers[0].pokemon = vec![TrainerPokemon::new(1, 10); 5];
    assert_eq!(issues(&g, vec![team(5)]), []);
    assert_eq!(issues(&g, vec![team(6)])[0].severity, EditSeverity::Error);
    g.trainers[0].multi_battle = MultiBattleStatus::Potential;
    assert_eq!(issues(&g, vec![team(5)]), []);
    assert_eq!(issues(&g, vec![team(6)])[0].severity, EditSeverity::Warning);
}

#[test]
fn held_items_and_moves_need_room() {
    let mut g = game();
    g.trainer_held_item_additions = [true, true, false];
    g.trainer_custom_movesets = [true, true, false];
    let edit = trainer(vec![TrainerPokemonEdit {
        held_item: Some(1),
        ..member(10, Some([1, 0, 0, 0]))
    }]);
    let found = issues(&g, vec![edit.clone()]);
    assert_eq!(
        messages(&found),
        [
            "Trainer 7 (Joey): this trainer's Pokémon can't hold items: its team has no room for them",
            "Trainer 7 (Joey): this trainer's Pokémon can't have moves of their own: its team has no room for them",
        ]
    );
    // A team that has them keeps them.
    g.trainers[0].pokemon[0].held_item = Some(2);
    g.trainers[0].pokemon[0].moves = [1, 0, 0, 0];
    assert_eq!(issues(&g, vec![edit]), []);
}

#[test]
fn gen1_trainers_have_no_moves_of_their_own() {
    let mut g = old_game(1);
    g.trainer_custom_movesets = [false; 3];
    assert_eq!(
        messages(&issues(&g, vec![trainer(vec![member(10, Some([1, 0, 0, 0]))])])),
        ["Trainer 7 (Joey): this trainer's Pokémon can't have moves of their own: this game doesn't store them"]
    );
}

#[test]
fn levels_the_game_sets_stay() {
    let g = game();
    let mut r = rules(&g);
    if let Some(t) = r.trainers.trainers.first_mut() {
        t.levels = false;
    }
    let same = trainer(vec![member(5, None), member(6, None)]);
    assert_eq!(issues_with(&g, &r, vec![same]), []);
    assert_eq!(
        messages(&issues_with(&g, &r, vec![team(2)])),
        ["Trainer 7 (Joey): the game sets this trainer's levels, so they can't change"]
    );
}

#[test]
fn a_linked_slot_follows_its_starter() {
    let g = game();
    let mut r = rules(&g);
    r.starter_links = vec![StarterLink {
        starter: 1,
        trainer: 7,
        slot: 0,
    }];
    let rival = |species| {
        trainer(vec![
            TrainerPokemonEdit {
                species,
                ..member(5, None)
            },
            member(6, None),
        ])
    };
    // Starter 2 is species 4 as loaded.
    assert_eq!(issues_with(&g, &r, vec![rival(4)]), []);
    assert_eq!(
        messages(&issues_with(&g, &r, vec![rival(5)])),
        ["Trainer 7 (Joey): slot 1 follows the rival's starter (S4); change it under starter 2"]
    );
    let starter = Edit::Starter(StarterEdit { slot: 1, species: 5 });
    assert_eq!(issues_with(&g, &r, vec![rival(5), starter]), []);
}

#[test]
fn wild_levels_follow_how_the_set_stores_them() {
    let g = game();
    let mut r = rules(&g);
    r.wild.levels = vec![WildLevels::Single];
    assert_eq!(issues_with(&g, &r, vec![wild(&[(4, 4), (5, 5)])]), []);
    assert_eq!(issues_with(&g, &r, vec![wild(&[(4, 5), (5, 5)])]).len(), 1);
    r.wild.levels = vec![WildLevels::Shared];
    assert_eq!(issues_with(&g, &r, vec![wild(&[(4, 6), (4, 6)])]), []);
    assert_eq!(
        messages(&issues_with(&g, &r, vec![wild(&[(4, 6), (4, 7)])])),
        [
            "Wild set 1 (ROUTE 1): the slots here share one level range, so every slot needs the same lowest and \
             highest level"
        ]
    );
    r.wild.levels = vec![WildLevels::Fixed];
    assert_eq!(issues_with(&g, &r, vec![wild(&[(2, 3), (2, 3)])]), []);
    assert_eq!(issues_with(&g, &r, vec![wild(&[(2, 4), (2, 3)])]).len(), 1);
}

#[test]
fn banned_wild_species_are_errors() {
    let mut g = game();
    g.wild_banned = vec![2, 1];
    g.encounters[0].banned = vec![3];
    let slots = |a, b| {
        Edit::WildSet(WildSetEdit {
            index: 0,
            slots: [a, b]
                .map(|species| WildSlotEdit {
                    species,
                    min_level: 2,
                    max_level: 3,
                })
                .to_vec(),
        })
    };
    assert_eq!(
        messages(&issues(&g, vec![slots(2, 3)])),
        [
            "Wild set 1 (ROUTE 1): S2 can't appear here in this game",
            "Wild set 1 (ROUTE 1): S3 can't appear here in this game",
        ]
    );
    assert_eq!(issues(&g, vec![slots(2, 2)]).len(), 1, "one issue per species");
    assert_eq!(issues(&g, vec![slots(1, 1)]), [], "the set has species 1 itself");
}

#[test]
fn trade_texts_must_fit() {
    let mut g = game();
    g.max_trade_nickname_length = 5;
    g.max_trade_ot_name_length = 3;
    let edit = |nickname: &str, ot: &str| {
        Edit::Trade(TradeEdit {
            nickname: nickname.into(),
            ot_name: ot.into(),
            ..match trade("ASH", vec![0; 6], None) {
                Edit::Trade(t) => t,
                _ => unreachable!(),
            }
        })
    };
    assert_eq!(issues(&g, vec![edit("ABCDE", "ABC")]), []);
    assert_eq!(
        messages(&issues(&g, vec![edit("ABCDEF", "ABCD")])),
        [
            "Trade 1 (BUDDY): the nickname takes 6 characters, but this game has room for 5",
            "Trade 1 (BUDDY): the original trainer's name takes 4 characters, but this game has room for 3",
        ]
    );
}

#[test]
fn trade_texts_the_game_has_always_fit() {
    let mut g = game();
    g.max_trade_nickname_length = 3;
    g.max_trade_ot_name_length = 3;
    // The game's own BUDDY and KATE are longer than the limits, but stay.
    let edit = Edit::Trade(TradeEdit {
        ot_name: "KATE".into(),
        ..match trade("ASH", vec![0; 6], None) {
            Edit::Trade(t) => t,
            _ => unreachable!(),
        }
    });
    assert_eq!(issues(&g, vec![edit]), []);
}

#[test]
fn trade_texts_of_a_game_that_keeps_them_cannot_change() {
    let g = game();
    let mut r = rules(&g);
    r.trades.texts = false;
    let edit = |nickname: &str, ot: &str| {
        Edit::Trade(TradeEdit {
            nickname: nickname.into(),
            ot_name: ot.into(),
            ..match trade("ASH", vec![0; 6], None) {
                Edit::Trade(t) => t,
                _ => unreachable!(),
            }
        })
    };
    let found = |e| issues_with(&g, &r, vec![e]);
    assert_eq!(found(edit("BUDDY", "KATE")), [], "the game's own texts");
    assert_eq!(
        messages(&found(edit("PAL", "ASH"))),
        [
            "Trade 1 (BUDDY): this game's trade nicknames can't change",
            "Trade 1 (BUDDY): this game's trade original trainer names can't change",
        ]
    );
}

#[test]
fn a_trade_that_asks_for_a_species_keeps_asking() {
    let g = game();
    let edit = match trade("ASH", vec![0; 6], None) {
        Edit::Trade(t) => Edit::Trade(TradeEdit { requested: None, ..t }),
        other => other,
    };
    assert_eq!(
        messages(&issues(&g, vec![edit])),
        ["Trade 1 (BUDDY): this trade needs a species to ask for: the game can't take any Pokémon"]
    );
}

#[test]
fn random_trade_ivs_are_kept() {
    for (generation, random) in [(5, 255), (6, 32), (7, 255)] {
        let mut g = game();
        g.identity.generation = Generation::new(generation).expect("valid");
        g.trades[0].ivs = vec![random; 6];
        assert_eq!(rules(&g).trades.random_iv, Some(random));
        let edit = trade("ASH", vec![random, 31, 0, random, 1, 2], None);
        assert_eq!(issues(&g, vec![edit]), [], "Gen {generation}");
    }
    assert!(
        !errors(trade("ASH", vec![32; 6], None)).is_empty(),
        "Gen 3 has no random IV"
    );
}

#[test]
fn replacing_a_required_field_item_is_a_warning() {
    let mut g = game();
    g.required_field_tms = vec![4];
    let found = issues(&g, vec![Edit::FieldItem(FieldItemEdit { index: 1, item: 7 })]);
    assert_eq!(
        messages(&found),
        ["Field item 2 (Item4): Item4 is only found here, so replacing it leaves the player without it"]
    );
    assert_eq!(found[0].severity, EditSeverity::Warning);
    let kept = Edit::FieldItem(FieldItemEdit { index: 1, item: 4 });
    assert_eq!(issues(&g, vec![kept]), []);
}

#[test]
fn gen1_special_defense_stays_empty() {
    let mut g = old_game(1);
    for s in &mut g.species {
        s.stats.sp_defense = 0;
    }
    let edit = |sp_defense| {
        Edit::Species(SpeciesEdit {
            number: 4,
            primary_type: Type::Fire,
            secondary_type: None,
            stats: StatsEdit {
                sp_defense,
                ..stats(90)
            },
            abilities: Vec::new(),
        })
    };
    assert_eq!(issues(&g, vec![edit(0)]), []);
    assert_eq!(
        messages(&issues(&g, vec![edit(90)])),
        ["Species #4 (S4): this game has no Special Defense"]
    );
}

#[test]
fn objects_are_named_by_place_and_name() {
    let g = game();
    assert_eq!(object_name(&g, ObjectRef::Starter { slot: 0 }), "Starter 1 (S1)");
    assert_eq!(
        object_name(&g, ObjectRef::Static { index: 0 }),
        "Static encounter 1 (S3)"
    );
    assert_eq!(object_name(&g, ObjectRef::ItemPrice { item: 2 }), "Price of Item2");
    assert_eq!(object_name(&g, ObjectRef::Shop { index: 5 }), "Shop 6");
}

#[test]
fn rules_and_issues_serialize_in_camel_case() {
    let g = game();
    let json = serde_json::to_value(rules(&g)).expect("serializes");
    assert_eq!(
        json["shops"]["size"],
        serde_json::json!({ "kind": "grow", "max": null })
    );
    assert!(json["trainers"]["trainers"][0].get("maxPokemon").is_some());
    let shared = ShopSize::Shared {
        bytes: 1,
        costs: Vec::new(),
        only_changed: true,
        max: 2,
    };
    let json = serde_json::to_value(shared).expect("serializes");
    assert_eq!(json["onlyChanged"], true);
    let issue = &issues(&g, vec![team(7)])[0];
    let json = serde_json::to_value(issue).expect("serializes");
    assert_eq!(json["severity"], "error");
    assert_eq!(json["target"], serde_json::json!({ "kind": "trainer", "index": 7 }));
}
