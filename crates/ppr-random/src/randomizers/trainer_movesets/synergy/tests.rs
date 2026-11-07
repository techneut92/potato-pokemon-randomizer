use ppr_model::{StatChange, Type};

use super::*;

/// No moves picked.
const NONE: [MoveId; 0] = [];

fn mv(number: MoveId, power: u16, move_type: Type, category: MoveCategory) -> Move {
    Move::basic(number, "M", power, move_type, category)
}

fn attack(number: MoveId, move_type: Type) -> Move {
    mv(number, 80, move_type, MoveCategory::Physical)
}

fn status(number: MoveId, move_type: Type) -> Move {
    mv(number, 0, move_type, MoveCategory::Status)
}

fn causing(number: MoveId, s: StatusType) -> Move {
    let mut m = status(number, Type::Normal);
    m.status = Some(s);
    m.status_move_type = StatusMoveType::NoDamage;
    m
}

fn with_stats(mut m: Move, kind: StatChangeMoveType, changes: &[(StatChangeType, i8, u8)]) -> Move {
    m.stat_change_move_type = kind;
    m.stat_changes = changes
        .iter()
        .map(|&(stat, stages, percent_chance)| StatChange {
            stat,
            stages,
            percent_chance,
        })
        .collect();
    m
}

fn numbers(moves: &[&Move]) -> Vec<MoveId> {
    moves.iter().map(|m| m.number).collect()
}

#[test]
fn soft_ability_synergy_boosts_damaging_moves_of_the_type() {
    let (surf, splash, ember) = (
        attack(57, Type::Water),
        status(150, Type::Water),
        attack(52, Type::Fire),
    );
    let moves = [&ember, &surf, &splash];
    assert_eq!(
        numbers(&soft_ability_synergy(a::DRIZZLE, &moves, Type::Normal, None)),
        [57]
    );
    assert_eq!(
        numbers(&soft_ability_synergy(a::DROUGHT, &moves, Type::Normal, None)),
        [52]
    );
    assert_eq!(
        numbers(&soft_ability_synergy(a::SPEED_BOOST, &moves, Type::Normal, None)),
        NONE
    );
}

#[test]
fn soft_ability_synergy_of_ate_abilities_needs_the_type() {
    let (tackle, growl) = (attack(33, Type::Normal), status(45, Type::Normal));
    let moves = [&tackle, &growl];
    assert_eq!(
        numbers(&soft_ability_synergy(a::PIXILATE, &moves, Type::Normal, None)),
        NONE
    );
    assert_eq!(
        numbers(&soft_ability_synergy(
            a::PIXILATE,
            &moves,
            Type::Normal,
            Some(Type::Fairy)
        )),
        [33]
    );
    assert_eq!(
        numbers(&soft_ability_synergy(a::REFRIGERATE, &moves, Type::Ice, None)),
        [33]
    );
}

#[test]
fn gale_wings_takes_flying_status_moves_too() {
    let (fly, roost) = (attack(m::FLY, Type::Flying), status(355, Type::Flying));
    let moves = [&roost, &fly];
    assert_eq!(
        numbers(&soft_ability_synergy(a::GALE_WINGS, &moves, Type::Flying, None)),
        [355, m::FLY]
    );
}

#[test]
fn soft_ability_anti_synergy_weakens_the_other_weather_type() {
    let (surf, ember, outrage) = (
        attack(57, Type::Water),
        attack(52, Type::Fire),
        attack(m::OUTRAGE, Type::Dragon),
    );
    let moves = [&surf, &ember, &outrage];
    assert_eq!(numbers(&soft_ability_anti_synergy(a::DRIZZLE, &moves)), [52]);
    assert_eq!(numbers(&soft_ability_anti_synergy(a::DROUGHT, &moves)), [57]);
    assert_eq!(
        numbers(&soft_ability_anti_synergy(a::MISTY_SURGE, &moves)),
        [m::OUTRAGE]
    );
}

#[test]
fn hard_ability_synergy_fixed_moves_follow_the_move_list_order() {
    let thunder = attack(m::THUNDER, Type::Electric);
    let ball = mv(m::WEATHER_BALL, 50, Type::Normal, MoveCategory::Special);
    let hurricane = attack(m::HURRICANE, Type::Flying);
    let moves = [&ball, &hurricane, &thunder, &thunder];
    let got = hard_ability_synergy(a::DRIZZLE, Type::Water, None, &moves, 3, 0);
    assert_eq!(numbers(&got), [m::WEATHER_BALL, m::HURRICANE, m::THUNDER]);
    let got = hard_ability_synergy(a::DRIZZLE, Type::Normal, None, &moves, 3, 0);
    assert_eq!(numbers(&got), [m::HURRICANE, m::THUNDER]);
}

#[test]
fn sturdy_and_drought_depend_on_the_generation() {
    let (flail, growth) = (attack(m::FLAIL, Type::Normal), status(m::GROWTH, Type::Normal));
    let moves = [&flail, &growth];
    assert_eq!(
        numbers(&hard_ability_synergy(a::STURDY, Type::Rock, None, &moves, 4, 0)),
        NONE
    );
    assert_eq!(
        numbers(&hard_ability_synergy(a::STURDY, Type::Rock, None, &moves, 5, 0)),
        [m::FLAIL]
    );
    assert_eq!(
        numbers(&hard_ability_synergy(a::DROUGHT, Type::Fire, None, &moves, 4, 0)),
        NONE
    );
    assert_eq!(
        numbers(&hard_ability_synergy(a::DROUGHT, Type::Fire, None, &moves, 5, 0)),
        [m::GROWTH]
    );
}

#[test]
fn poison_point_falls_through_to_hex() {
    let (venoshock, hex) = (attack(m::VENOSHOCK, Type::Poison), attack(m::HEX, Type::Ghost));
    let moves = [&hex, &venoshock];
    let got = hard_ability_synergy(a::POISON_POINT, Type::Poison, None, &moves, 5, 0);
    assert_eq!(numbers(&got), [m::HEX, m::VENOSHOCK]);
    let got = hard_ability_synergy(a::FLAME_BODY, Type::Fire, None, &moves, 5, 0);
    assert_eq!(numbers(&got), [m::HEX]);
}

#[test]
fn hustle_takes_accuracy_boosts_and_perfect_physical_moves() {
    let mut swift = attack(m::FLY + 1000, Type::Normal);
    swift.accuracy = 0;
    let tackle = attack(33, Type::Normal);
    let hone = with_stats(
        status(468, Type::Dark),
        StatChangeMoveType::NoDamageUser,
        &[(StatChangeType::Attack, 1, 0), (StatChangeType::Accuracy, 1, 0)],
    );
    let moves = [&tackle, &swift, &hone];
    assert_eq!(
        numbers(&hard_ability_synergy(HUSTLE, Type::Bug, None, &moves, 3, 0)),
        [swift.number, 468]
    );
}

#[test]
fn skill_link_and_technician_use_the_hit_count() {
    let mut fury = mv(31, 15, Type::Normal, MoveCategory::Physical);
    fury.hit_count_hundredths = 300;
    let mut double = mv(24, 30, Type::Fighting, MoveCategory::Physical);
    double.hit_count_hundredths = 200;
    let quick = mv(98, 40, Type::Normal, MoveCategory::Physical);
    let moves = [&fury, &double, &quick];
    assert_eq!(
        numbers(&hard_ability_synergy(a::SKILL_LINK, Type::Normal, None, &moves, 3, 0)),
        [31]
    );
    assert_eq!(
        numbers(&hard_ability_synergy(a::TECHNICIAN, Type::Normal, None, &moves, 3, 0)),
        [31, 24, 98]
    );
}

#[test]
fn serene_grace_and_sheer_force_take_secondary_effects() {
    let mut bite = attack(m::BITE, Type::Dark);
    bite.flinch_percent_chance = 30;
    let acid = with_stats(
        attack(51, Type::Poison),
        StatChangeMoveType::DamageTarget,
        &[(StatChangeType::SpecialDefense, -1, 10)],
    );
    let overheat = with_stats(
        attack(315, Type::Fire),
        StatChangeMoveType::DamageUser,
        &[(StatChangeType::SpecialAttack, -2, 100)],
    );
    let tackle = attack(33, Type::Normal);
    let moves = [&tackle, &bite, &acid, &overheat];
    assert_eq!(
        numbers(&hard_ability_synergy(a::SERENE_GRACE, Type::Normal, None, &moves, 3, 0)),
        [m::BITE, 51]
    );
    assert_eq!(
        numbers(&hard_ability_synergy(a::SHEER_FORCE, Type::Normal, None, &moves, 5, 0)),
        [m::BITE, 51]
    );
    assert_eq!(
        numbers(&hard_ability_synergy(a::CONTRARY, Type::Normal, None, &moves, 5, 0)),
        [315]
    );
}

#[test]
fn synergy_ability_maps_gen3_air_lock() {
    assert_eq!(synergy_ability(77, 3), AIR_LOCK);
    assert_eq!(synergy_ability(76, 3), 0);
    assert_eq!(synergy_ability(77, 4), a::TANGLED_FEET);
    assert_eq!(synergy_ability(a::DRIZZLE, 3), a::DRIZZLE);
    let (rain, outrage) = (status(m::RAIN_DANCE, Type::Water), attack(m::OUTRAGE, Type::Dragon));
    let moves = [&rain, &outrage];
    let rayquaza = synergy_ability(77, 3);
    assert_eq!(
        numbers(&hard_ability_synergy(
            rayquaza,
            Type::Dragon,
            Some(Type::Flying),
            &moves,
            3,
            0
        )),
        NONE
    );
    assert_eq!(numbers(&hard_ability_anti_synergy(rayquaza, &moves)), [m::RAIN_DANCE]);
}

#[test]
fn hard_ability_anti_synergy_of_weather() {
    let solar = mv(m::SOLAR_BEAM, 120, Type::Grass, MoveCategory::Special);
    let thunder = attack(m::THUNDER, Type::Electric);
    let ember = attack(52, Type::Fire);
    let surf = attack(57, Type::Water);
    let moves = [&solar, &thunder, &ember, &surf];
    assert_eq!(numbers(&hard_ability_anti_synergy(a::DRIZZLE, &moves)), [m::SOLAR_BEAM]);
    assert_eq!(
        numbers(&hard_ability_anti_synergy(a::PRIMORDIAL_SEA, &moves)),
        [m::SOLAR_BEAM, 52]
    );
    assert_eq!(numbers(&hard_ability_anti_synergy(a::DROUGHT, &moves)), [m::THUNDER]);
    assert_eq!(
        numbers(&hard_ability_anti_synergy(a::DESOLATE_LAND, &moves)),
        [m::THUNDER, 57]
    );
}

#[test]
fn hard_ability_anti_synergy_of_stat_abilities() {
    let agility = with_stats(
        status(97, Type::Psychic),
        StatChangeMoveType::NoDamageUser,
        &[(StatChangeType::Speed, 2, 0)],
    );
    let overheat = with_stats(
        attack(315, Type::Fire),
        StatChangeMoveType::DamageUser,
        &[(StatChangeType::SpecialAttack, -2, 100)],
    );
    let smash = status(m::SHELL_SMASH, Type::Normal);
    let sand = with_stats(
        status(28, Type::Ground),
        StatChangeMoveType::NoDamageTarget,
        &[(StatChangeType::Accuracy, -1, 0)],
    );
    let moves = [&agility, &overheat, &smash, &sand];
    assert_eq!(numbers(&hard_ability_anti_synergy(a::SPEED_BOOST, &moves)), [97]);
    assert_eq!(numbers(&hard_ability_anti_synergy(a::SIMPLE, &moves)), [315]);
    assert_eq!(
        numbers(&hard_ability_anti_synergy(a::CONTRARY, &moves)),
        [97, m::SHELL_SMASH]
    );
    assert_eq!(numbers(&hard_ability_anti_synergy(a::NO_GUARD, &moves)), NONE);
}

#[test]
fn misty_surge_spoils_status_but_not_confusion() {
    let (spore, confuse, rest) = (
        causing(147, StatusType::Sleep),
        causing(109, StatusType::Confusion),
        status(m::REST, Type::Psychic),
    );
    let moves = [&spore, &confuse, &rest];
    assert_eq!(
        numbers(&hard_ability_anti_synergy(a::MISTY_SURGE, &moves)),
        [147, m::REST]
    );
    assert_eq!(
        numbers(&hard_ability_anti_synergy(a::ELECTRIC_SURGE, &moves)),
        [147, m::REST]
    );
}

fn stats(hp: u8, attack: u8, defense: u8, speed: u8, sp_attack: u8, sp_defense: u8) -> BaseStats {
    BaseStats {
        hp,
        attack,
        defense,
        speed,
        sp_attack,
        sp_defense,
    }
}

#[test]
fn stat_synergy_follows_the_ratios() {
    let ids = [
        m::PAIN_SPLIT,
        m::WATER_SPOUT,
        m::POWER_TRICK,
        m::GYRO_BALL,
        m::ELECTRO_BALL,
        m::POWER_SPLIT,
        m::GUARD_SPLIT,
    ];
    let all: Vec<Move> = ids.iter().map(|&n| attack(n, Type::Normal)).collect();
    let moves: Vec<&Move> = all.iter().collect();
    // Shuckle: little HP and speed, attack far below defense, weak attacks.
    let shuckle = stats(20, 10, 230, 5, 10, 230);
    assert_eq!(
        numbers(&stat_synergy(&shuckle, &moves)),
        [m::PAIN_SPLIT, m::POWER_TRICK, m::GYRO_BALL, m::POWER_SPLIT]
    );
    assert_eq!(
        numbers(&stat_anti_synergy(&shuckle, &moves)),
        [m::ELECTRO_BALL, m::GUARD_SPLIT]
    );
    // Exactly a quarter is "at least a quarter": 100 of 400.
    let bulky_fast = stats(100, 50, 50, 100, 50, 50);
    assert_eq!(
        numbers(&stat_synergy(&bulky_fast, &moves)),
        [m::WATER_SPOUT, m::ELECTRO_BALL]
    );
    assert_eq!(
        numbers(&stat_anti_synergy(&bulky_fast, &moves)),
        [m::PAIN_SPLIT, m::GYRO_BALL]
    );
}

#[test]
fn move_synergy_pairs_speed_control_with_flinching() {
    let mut bite = attack(m::BITE, Type::Dark);
    bite.flinch_percent_chance = 30;
    let mut fake_out = attack(m::FAKE_OUT, Type::Normal);
    fake_out.flinch_percent_chance = 100;
    fake_out.priority = 3;
    let icy_wind = with_stats(
        attack(196, Type::Ice),
        StatChangeMoveType::DamageTarget,
        &[(StatChangeType::Speed, -1, 100)],
    );
    let moves = [&fake_out, &bite, &icy_wind];
    assert_eq!(numbers(&move_synergy(&icy_wind, &moves, 3)), [m::BITE]);
    assert_eq!(numbers(&move_synergy(&bite, &moves, 3)), [196]);
    assert_eq!(numbers(&move_synergy(&fake_out, &moves, 3)), NONE);
}

#[test]
fn move_synergy_of_toxic_falls_through_to_venoshock() {
    let toxic = causing(m::TOXIC, StatusType::ToxicPoison);
    let mut wrap = attack(35, Type::Normal);
    wrap.is_trap = true;
    let (protect, venoshock, hex) = (
        status(m::PROTECT, Type::Normal),
        attack(m::VENOSHOCK, Type::Poison),
        attack(m::HEX, Type::Ghost),
    );
    let moves = [&hex, &wrap, &venoshock, &protect, &toxic];
    assert_eq!(
        numbers(&move_synergy(&toxic, &moves, 3)),
        [m::HEX, 35, m::VENOSHOCK, m::PROTECT]
    );
    let powder = causing(m::POISON_POWDER, StatusType::Poison);
    assert_eq!(numbers(&move_synergy(&powder, &moves, 3)), [m::HEX, m::VENOSHOCK]);
    assert_eq!(numbers(&move_synergy(&venoshock, &moves, 3)), [m::TOXIC]);
}

#[test]
fn move_synergy_of_setup_and_sleep() {
    let dance = with_stats(
        status(m::SWORDS_DANCE, Type::Normal),
        StatChangeMoveType::NoDamageUser,
        &[(StatChangeType::Attack, 2, 0)],
    );
    let (pass, stored) = (
        status(m::BATON_PASS, Type::Normal),
        attack(m::STORED_POWER, Type::Psychic),
    );
    let spore = causing(147, StatusType::Sleep);
    let eater = attack(m::DREAM_EATER, Type::Psychic);
    let moves = [&eater, &stored, &pass, &dance, &spore];
    assert_eq!(
        numbers(&move_synergy(&dance, &moves, 3)),
        [m::STORED_POWER, m::BATON_PASS]
    );
    assert_eq!(numbers(&move_synergy(&pass, &moves, 3)), [m::SWORDS_DANCE]);
    assert_eq!(numbers(&move_synergy(&spore, &moves, 3)), [m::DREAM_EATER]);
    assert_eq!(numbers(&move_synergy(&eater, &moves, 3)), [147]);
}

#[test]
fn lock_on_skips_moves_that_never_miss() {
    let mut zap = attack(192, Type::Electric);
    zap.accuracy = 50;
    let mut swift = attack(129, Type::Normal);
    swift.accuracy = 0;
    let lock_on = status(m::LOCK_ON, Type::Normal);
    let moves = [&swift, &zap];
    assert_eq!(numbers(&move_synergy(&lock_on, &moves, 3)), [192]);
}

#[test]
fn soft_move_synergy_covers_resisting_types() {
    let mut chart = TypeChart::neutral(&[Type::Normal, Type::Rock, Type::Ghost, Type::Fighting, Type::Water]);
    chart.set(Type::Normal, Type::Rock, Effectiveness::Half);
    chart.set(Type::Normal, Type::Ghost, Effectiveness::Zero);
    chart.set(Type::Fighting, Type::Rock, Effectiveness::Double);
    chart.set(Type::Ghost, Type::Ghost, Effectiveness::Double);
    let tackle = attack(33, Type::Normal);
    let (karate, lick, surf) = (
        attack(2, Type::Fighting),
        attack(122, Type::Ghost),
        attack(57, Type::Water),
    );
    let spite = status(180, Type::Ghost);
    let moves = [&surf, &lick, &spite, &karate, &tackle];
    assert_eq!(numbers(&soft_move_synergy(&tackle, &moves, &chart)), [122, 2]);
    assert_eq!(numbers(&soft_move_synergy(&spite, &moves, &chart)), NONE);
    let dance = status(m::SWORDS_DANCE, Type::Normal);
    assert_eq!(numbers(&soft_move_synergy(&dance, &moves, &chart)), [57, 122, 2, 33]);
}

#[test]
fn hard_move_anti_synergy_of_status_and_protect() {
    let (hypnosis, glare, confuse) = (
        causing(147, StatusType::Sleep),
        causing(m::GLARE, StatusType::Paralyze),
        causing(109, StatusType::Confusion),
    );
    let (protect, detect) = (status(m::PROTECT, Type::Normal), status(m::DETECT, Type::Fighting));
    let moves = [&hypnosis, &glare, &confuse, &protect, &detect];
    assert_eq!(numbers(&hard_move_anti_synergy(&hypnosis, &moves)), [147, m::GLARE]);
    assert_eq!(numbers(&hard_move_anti_synergy(&confuse, &moves)), [109]);
    assert_eq!(numbers(&hard_move_anti_synergy(&protect, &moves)), [m::DETECT]);
    let (ember, water_sport) = (attack(52, Type::Fire), status(m::WATER_SPORT, Type::Water));
    assert_eq!(
        numbers(&hard_move_anti_synergy(&ember, &[&water_sport, &ember])),
        [m::WATER_SPORT]
    );
}

#[test]
fn soft_move_anti_synergy_of_same_type_and_sports() {
    let (ember, flame, surf) = (attack(52, Type::Fire), attack(53, Type::Fire), attack(57, Type::Water));
    let water_sport = status(m::WATER_SPORT, Type::Water);
    let moves = [&surf, &flame, &ember, &water_sport];
    assert_eq!(numbers(&soft_move_anti_synergy(&ember, &moves)), [53, 52]);
    assert_eq!(numbers(&soft_move_anti_synergy(&water_sport, &moves)), [53, 52]);
}

#[test]
fn requires_other_move_finds_the_enablers() {
    let (stockpile, spit, rest, snore) = (
        status(m::STOCKPILE, Type::Normal),
        attack(m::SPIT_UP, Type::Normal),
        status(m::REST, Type::Psychic),
        attack(m::SNORE, Type::Normal),
    );
    let (hypnosis, dream_eater) = (causing(147, StatusType::Sleep), attack(m::DREAM_EATER, Type::Psychic));
    let moves = [&spit, &rest, &stockpile, &hypnosis];
    assert_eq!(numbers(&requires_other_move(&spit, &moves)), [m::STOCKPILE]);
    assert_eq!(numbers(&requires_other_move(&snore, &moves)), [m::REST]);
    assert_eq!(numbers(&requires_other_move(&dream_eater, &moves)), [147]);
    assert_eq!(numbers(&requires_other_move(&dream_eater, &[&spit])), NONE);
    assert_eq!(numbers(&requires_other_move(&rest, &moves)), NONE);
}

#[test]
fn global_lists_hold_expected_move_numbers() {
    assert_eq!(USELESS_MOVES, [150, 606, 607, 100, 513]);
    assert_eq!(REQUIRES_OTHER_MOVE, [255, 256, 138, 171]);
    assert!(DOUBLE_BATTLE_MOVES.contains(&270));
    assert!(CANNOT_OBSOLETE_MOVES.contains(&153));
    assert!(CANNOT_BE_OBSOLETED_MOVES.contains(&216));
}
