//! Test games for the randomizers' tests.

use ppr_model::{BaseStats, DexNumber, GameData, Generation, Platform, RomIdentity, Species, Type, TypeChart};

/// A species with every base stat `each`, called `Mon<number>`.
pub(super) fn species(number: DexNumber, primary: Type, secondary: Option<Type>, each: u8) -> Species {
    Species {
        number,
        name: format!("Mon{number}"),
        primary_type: primary,
        secondary_type: secondary,
        stats: BaseStats {
            hp: each,
            attack: each,
            defense: each,
            speed: each,
            sp_attack: each,
            sp_defense: each,
        },
        catch_rate: 45,
        abilities: Vec::new(),
        growth_curve: ppr_model::ExpCurve::MediumFast,
        forme: ppr_model::Forme::default(),
    }
}

/// Alternate forme `forme` of `base`, numbered `number`, with every base stat `each`; `cosmetic` and `irregular` as
/// the forme's flags.
pub(super) fn alt_forme(
    number: DexNumber,
    base: DexNumber,
    forme: u8,
    each: u8,
    cosmetic: bool,
    irregular: bool,
) -> Species {
    let mut sp = species(number, Type::Normal, None, each);
    sp.forme.base = Some(base);
    sp.forme.number = forme;
    sp.forme.cosmetic = cosmetic;
    sp.forme.irregular = irregular;
    sp
}

/// An Emerald with these species, Gen 3's types and a neutral type chart, and nothing else.
pub(super) fn emerald(species: Vec<Species>) -> GameData {
    let generation = Generation::new(3).expect("valid");
    let types = Type::in_generation(generation);
    GameData {
        formes: ppr_model::FormeRules::default(),
        identity: RomIdentity {
            name: "Emerald (U)".into(),
            code: "BPEE".into(),
            version: Some(0),
            generation,
            platform: Platform::GameBoyAdvance,
            family: "Em".into(),
        },
        type_chart: TypeChart::neutral(&types),
        types,
        species,
        evolutions: Vec::new(),
        evolution_level_samples: Vec::new(),
        restricted_species: None,
        starters: vec![1, 4, 7],
        starter_type_triangle: None,
        starter_held_items: vec![None],
        items: Vec::new(),
        wild_held_items: std::collections::BTreeMap::new(),
        guaranteed_wild_held_items: true,
        dark_grass_held_items: false,
        palettes: std::collections::BTreeMap::new(),
        palette_files: None,
        player_graphics_packs: Vec::new(),
        player_graphics: None,
        encounters: Vec::new(),
        guaranteed_catching: false,
        wild_banned: Vec::new(),
        statics: Vec::new(),
        main_game_legendaries: Vec::new(),
        special_music_statics: Vec::new(),
        correct_static_music: false,
        totems: Vec::new(),
        easier_happiness_evolutions: false,
        moves: Vec::new(),
        max_move_name_length: 0,
        learnsets: std::collections::BTreeMap::new(),
        egg_moves: std::collections::BTreeMap::new(),
        hm_moves: Vec::new(),
        game_breaking_moves: Vec::new(),
        banned_level_up_moves: Vec::new(),
        four_starting_moves: false,
        tm_moves: Vec::new(),
        tutor_moves: Vec::new(),
        tm_hm_compat: std::collections::BTreeMap::new(),
        tutor_compat: std::collections::BTreeMap::new(),
        field_items: Vec::new(),
        required_field_tms: Vec::new(),
        shops: Vec::new(),
        item_prices: std::collections::BTreeMap::new(),
        balanced_item_prices: std::collections::BTreeMap::new(),
        op_shop_items: Vec::new(),
        mega_stones: Vec::new(),
        evolution_items: Vec::new(),
        pickup_items: Vec::new(),
        pickup_tms: false,
        early_required_hm_moves: Vec::new(),
        field_moves: Vec::new(),
        trades: Vec::new(),
        max_trade_nickname_length: 10,
        max_trade_ot_name_length: 7,
        ability_names: Vec::new(),
        ability_variations: std::collections::BTreeMap::new(),
        useless_abilities: Vec::new(),
        trainers: Vec::new(),
        trainer_class_names: Vec::new(),
        elite_four_trainers: Vec::new(),
        gym_and_elite_themes: std::collections::BTreeMap::new(),
        original_types: std::collections::BTreeMap::new(),
        max_trainer_name_length: 11,
        max_trainer_class_name_length: 12,
        trainer_name_shares_class_line: false,
        max_trainer_names_total: None,
        doubles_trainer_classes: Vec::new(),
        personal_trainer_classes: Vec::new(),
        fixed_trainer_text: false,
        fixed_trainer_name_length: false,
        fixed_trainer_class_name_length: false,
        text_tokens: Vec::new(),
        intro_candidates: Vec::new(),
        intro_pokemon: None,
        held_item_pools: ppr_model::HeldItemPools::default(),
        trainer_pokemon_additions: [true; 3],
        trainer_custom_movesets: [true; 3],
        trainer_held_item_additions: [true; 3],
        available_tweaks: Vec::new(),
        applied_tweaks: std::collections::BTreeSet::new(),
        pc_potion: None,
        catching_tutorial_opponents: Vec::new(),
        catching_tutorial_players: Vec::new(),
        catching_tutorial: None,
        fossils: Vec::new(),
    }
}

/// A Gen 1 game with these species, Gen 1's types and a neutral type chart: Yellow when `yellow`, else Red. Gen 1
/// has one Special stat, kept in `sp_attack`; `sp_defense` is 0.
pub(super) fn gen1(mut species: Vec<Species>, yellow: bool) -> GameData {
    let generation = Generation::new(1).expect("valid");
    let types = Type::in_generation(generation);
    for sp in &mut species {
        sp.stats.sp_defense = 0;
    }
    let mut g = emerald(species);
    g.identity = RomIdentity {
        name: if yellow { "Yellow (U)" } else { "Red (U)" }.into(),
        code: if yellow { "POKEMON YELLOW" } else { "POKEMON RED" }.into(),
        version: Some(0),
        generation,
        platform: Platform::GameBoy,
        family: if yellow { "Yellow" } else { "RB" }.into(),
    };
    g.type_chart = TypeChart::neutral(&types);
    g.types = types;
    g.starters = if yellow { vec![25, 133] } else { vec![1, 4, 7] };
    g.starter_held_items = Vec::new();
    g.trainer_pokemon_additions = [true, true, false];
    g.trainer_custom_movesets = [false; 3];
    g
}
