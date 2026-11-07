import type { Ctx, GroupDef, OptDef, OptKind, SectionDef } from './types';
import type { RomInfo } from '../data/roms';

// Option definitions, with the rules for when each option is enabled and shown for the loaded game and the other
// choices.

type X = Partial<OptDef>;
const O = (k: OptKind, label: string, x?: X): OptDef => ({ k, label, ...x });
const T = (label: string, on: boolean, x?: X) => O('toggle', label, { on, ...x });
const SL = (label: string, min: number, max: number, val: number, unit: string, x?: X) => O('slider', label, { min, max, val, unit, ...x });
const SEL = (label: string, choices: OptDef['choices'], val: number, x?: X) => O('select', label, { choices, val, ...x });
const NUM = (label: string, val: number, min: number, max: number, x?: X) => O('number', label, { val, min, max, ...x });
const RA = (label: string, choices: readonly string[], val: number, x?: X) => O('radio', label, { choices, val, ...x });
const H = (label: string): OptDef => ({ k: 'sub', label });
const I = (label: string): OptDef => ({ k: 'info', label });
const G = (title: string, key: string, modes: readonly string[] | null, mi: number, opts: OptDef[], x?: Partial<GroupDef>): GroupDef => ({ title, key, modes, mi, opts, ...x });
const notEL = (c: Ctx) => !c.el();
const free = true;
const compat = ['Unchanged', 'Random (prefer same type)', 'Random (completely)', 'Full Compatibility'];
const tier = (row: number, col: number, x?: X) => O('tier', '', { key: 't' + row + col, row, col, on: false, free, ...x });
const tpBasic = (c: Ctx) => c.m('tp') !== 0 || c.tpAdded();

export function sections(): SectionDef[] {
  return [
    { id: "general", label: "General", icon: "M4 7h9m4 0h3M4 17h3m4 0h9M15 5v4M9 15v4", desc: "Rules that apply across every category, plus what happens to the loaded game after saving.", groups: [
      G("General Options", "gen", null, 0, [
        O("limit", "Limit Pokémon", { key: "limit", on: false, free, t: "limitPokemonCheckBox" }),
        T("No Irregular Alt Formes", false, { key: "irr", free, vis: (r: RomInfo) => r.genNum >= 4, t: "noIrregularAltFormesCheckBox" }),
        T("No Premature Evolutions", false, { key: "prem", free, en: notEL, t: "noPrematureEvosCheckbox" }),
        T("No Random Intro Mon", false, { key: "intro", free, vis: (r: RomInfo) => r.intro, t: "noRandomIntroMonCheckBox" }),
        T("Race Mode", false, { key: "race", free, t: "raceModeCheckBox" })
      ]),
      G("Game Update", "upd", null, 0, [I("Load an update file so it is included in the randomized output. With an update loaded, the game is saved as a LayeredFS folder."), O("update", "update")], { tag: "3DS only", vis: (r: RomInfo) => r.genNum >= 6 })
    ]},
    { id: "traits", label: "Pokémon Traits", icon: "M12 3l2.6 5.6 6 .7-4.5 4.1 1.2 6L12 16.4 6.7 19.4l1.2-6L3.4 9.3l6-.7z", desc: "Base stats, stat totals, types, abilities, evolutions and EXP curves.", groups: [
      G("Base Statistics", "pbs", ["Unchanged", "Shuffle", "Random"], 0, [
        T("Follow Evolutions", false, { key: "fe", en: notEL, t: "pbsFollowEvolutionsCheckBox" }),
        T("Follow Mega Evolutions", false, { key: "fm", vis: (r: RomInfo) => r.mega, en: (c: Ctx) => c.megaOk(), t: "pbsFollowMegaEvosCheckBox" }),
        T("Randomize Added Stats on Evolution", false, { key: "ae", need: [2], en: (c: Ctx) => c.on("pbs.fe") || c.on("pbs.fm"), t: "pbsAssignEvoStatsRandomlyCheckBox" }),
        H("Updates"),
        SEL("Update Base Stats to Generation", "PBSGENS", 0, { key: "upd", tog: true, free, vis: (r: RomInfo) => r.genNum < 8, t: "pbsUpdateBaseStatsCheckBox" })
      ], { pfx: ["pbs"] }),
      G("Base Stat Totals", "pbst", ["Unchanged", "Random Buff/Nerf (%)", "Shuffle", "Random"], 0, [
        SL("Maximum buff/nerf", 0, 50, 0, "%", { key: "bn", need: [1], t: "pbstRandomBuffNerfSpinSlider" }),
        T("Follow Evolutions", false, { key: "fe", free, en: (c: Ctx) => (c.m("pbst") === 1 || (c.m("pbst") === 2 && c.m("pe") !== 1)) && !c.el(), t: "pbstFollowEvolutionsCheckBox" }),
        T("Separate Legendaries", false, { key: "sl", need: [2], t: "pbstSwapLegendariesCheckBox" })
      ], { pfx: ["pbst"] }),
      G("Types", "pt", ["Unchanged", "Random (follow evolutions)", "Random (completely)"], 0, [
        T("Follow Mega Evolutions", false, { key: "fm", vis: (r: RomInfo) => r.mega, en: (c: Ctx) => c.megaOk(), t: "ptFollowMegaEvosCheckBox" }),
        T("Force Dual Types", false, { key: "dual", en: (c: Ctx) => !c.on("sp.nodual"), t: "ptIsDualTypeCheckBox" })
      ], { pfx: ["pt"], men: (c: Ctx, i: number) => !(i === 1 && c.el()), mfall: 2 }),
      G("Abilities", "pa", ["Unchanged", "Random"], 0, [
        T("Allow Wonder Guard", false, { key: "wg", t: "paAllowWonderGuardCheckBox" }),
        T("Follow Evolutions", false, { key: "fe", en: notEL, t: "paFollowEvolutionsCheckBox" }),
        T("Follow Mega Evolutions", false, { key: "fm", vis: (r: RomInfo) => r.mega, en: (c: Ctx) => c.megaOk(), t: "paFollowMegaEvosCheckBox" }),
        T("Combine Duplicate Abilities", false, { key: "dup", t: "paWeighDuplicatesTogetherCheckBox" }),
        T("Ensure Two Abilities", false, { key: "two", t: "paEnsureTwoAbilitiesCheckbox" }),
        H("Ban…"),
        T("Trapping Abilities", false, { key: "trap", t: "paTrappingAbilitiesCheckBox" }),
        T("Negative Abilities", false, { key: "neg", t: "paNegativeAbilitiesCheckBox" }),
        T("Bad Abilities", false, { key: "bad", t: "paBadAbilitiesCheckBox" })
      ], { pfx: ["pa"], vis: (r: RomInfo) => r.genNum >= 3 }),
      G("Evolutions", "pe", ["Unchanged", "Random", "Random Every Level"], 0, [
        T("Similar Strength", false, { key: "ss", need: [1], t: "peSimilarStrengthCheckBox" }),
        T("Same Typing", false, { key: "st", need: [1, 2], t: "peSameTypingCheckBox" }),
        T("Limit Evolutions to Three Stages", false, { key: "l3", need: [1], t: "peLimitEvolutionsToThreeCheckBox" }),
        T("Force Change", false, { key: "fc", need: [1, 2], t: "peForceChangeCheckBox" }),
        T("Force Growth", false, { key: "fg", need: [1], t: "peForceGrowthCheckBox" }),
        T("No Convergence", false, { key: "nc", need: [1, 2], t: "peNoConvergenceCheckBox" }),
        T("Allow Alternate Formes", false, { key: "alt", need: [1, 2], vis: (r: RomInfo) => r.genNum >= 7, t: "peAllowAltFormesCheckBox" }),
        H("Always available"),
        T("Change Impossible Evolutions", false, { key: "ci", free, en: notEL, t: "peChangeImpossibleEvosCheckBox" }),
        SL("Make Evolutions Easier", 30, 55, 40, "", { key: "easy", tog: true, free, en: notEL, t: "peMakeEvolutionsEasierCheckBox", unitText: "Lv " }),
        T("Use Estimated Evolution Levels", false, { key: "est", free, en: (c: Ctx) => (c.on("pe.ci") || c.on("pe.easy")) && !c.el(), t: "peUseEstimatedInsteadOfHardcodedLevelsCheckBox" }),
        T("Remove Time-Based Evolutions", false, { key: "time", free, en: notEL, t: "peRemoveTimeBasedEvolutionsCheckBox" }),
        T("Adjust Evolution Levels", false, { key: "adj", free, en: (c: Ctx) => c.m("pe") === 1 || c.m("pbst") !== 0, t: "peAdjustLevelsCheckBox" })
      ], { pfx: ["pe"] }),
      G("EXP Curves", "exp", null, 0, [
        SEL("Standardize EXP Curves to", ["Medium Fast", "Erratic", "Fluctuating", "Medium Slow", "Fast", "Slow"], 0, { key: "std", tog: true, free, t: "pbsStandardizeEXPCurvesCheckBox" }),
        RA("Apply to", ["Legendaries: Slow", "Strong Legendaries: Slow", "All Pokémon"], 0, { key: "who", free, en: (c: Ctx) => c.on("exp.std"), pfx: ["pbsLegendariesSlow", "pbsStrongLegendariesSlow", "pbsAllMediumFast"], tips: ["pbsLegendariesSlowRadioButton", "pbsStrongLegendariesSlowRadioButton", "pbsAllMediumFastRadioButton"] })
      ])
    ]},
    { id: "starters", label: "Starters, Statics & Trades", icon: "M12 3c3.5 0 6.5 5.5 6.5 10a6.5 6.5 0 0 1-13 0C5.5 8.5 8.5 3 12 3z", desc: "Your first partner, gift and legendary encounters, and in-game trades.", groups: [
      G("Starter Pokémon", "sp", ["Unchanged", "Custom", "Random (completely)", "Random (basic Pokémon with 2 evolutions)", "Random (any basic Pokémon)"], 0, [
        H("Custom starters"),
        O("species", "Starter 1", { key: "s1", slot: 0, need: [1] }), O("species", "Starter 2", { key: "s2", slot: 1, need: [1] }), O("species", "Starter 3", { key: "s3", slot: 2, need: [1] }),
        H("Restrictions"),
        RA("Type restrictions", ["None", "Fire, Water, Grass", "Any Type Triangle", "Single Type", "Unique"], 0, { key: "type", free, en: (c: Ctx) => c.spBranch(), cen: (c: Ctx, ci: number) => !((ci === 1 || ci === 2) && c.spCustomRandom()), tips: ["spTypeNoneRadioButton", "spTypeFwgRadioButton", "spTypeTriangleRadioButton", "spTypeSingleRadioButton", "spTypeUniqueRadioButton"] }),
        SEL("Single type", "TYPES", 0, { key: "single", free, en: (c: Ctx) => c.spBranch() && c.v("sp.type") === 3, t: "spTypeSingleComboBox" }),
        T("No Dual Types", false, { key: "nodual", free, en: (c: Ctx) => c.spBranch() && !c.on("pt.dual"), t: "spTypeNoDualCheckbox" }),
        T("Don’t Use Legendaries", false, { key: "noleg", free, en: (c: Ctx) => c.spBranch(), t: "spNoLegendariesCheckBox" }),
        T("Allow Alternate Formes", false, { key: "alt", free, en: (c: Ctx) => c.spBranch(), vis: (r: RomInfo) => r.starterAlt, t: "spAllowAltFormesCheckBox" }),
        NUM("Minimum BST", 307, 1, 1530, { key: "bmin", tog: true, free, en: (c: Ctx) => c.spBranch(), t: "spBSTMinimumCheckbox" }),
        NUM("Maximum BST", 320, 1, 1530, { key: "bmax", tog: true, free, en: (c: Ctx) => c.spBranch(), t: "spBSTMaximumCheckbox" }),
        H("Held items"),
        T("Randomize Starter Held Items", false, { key: "held", free, vis: (r: RomInfo) => r.heldStarter, t: "spRandomizeStarterHeldItemsCheckBox" }),
        T("Ban Bad Items", false, { key: "ban", free, vis: (r: RomInfo) => r.heldStarter, en: (c: Ctx) => c.on("sp.held"), t: "spBanBadItemsCheckBox" })
      ], { wide: true, pfx: ["sp"], men: (c: Ctx, i: number) => !((i === 3 || i === 4) && c.el()), mfall: 2 }),
      G("Static Pokémon", "stp", ["Unchanged", "Random (legendary ↔ legendary)", "Random (completely)", "Random (similar strength)"], 0, [
        T("Limit Main-Game Legendaries", false, { key: "lim", need: [3], vis: (r: RomInfo) => r.mainLeg, t: "stpLimitMainGameLegendariesCheckBox" }),
        T("Randomize 600+ BST", false, { key: "bst", t: "stpRandomize600BSTCheckBox" }),
        T("Allow Alternate Formes", false, { key: "alt", vis: (r: RomInfo) => r.staticAlt, t: "stpAllowAltFormesCheckBox" }),
        T("Swap Mega Evolvables", false, { key: "mega", vis: (r: RomInfo) => r.genNum === 6, t: "stpSwapMegaEvosCheckBox" }),
        T("Fix Music", false, { key: "music", vis: (r: RomInfo) => r.musicFix, t: "stpFixMusicCheckBox" }),
        H("Always available"),
        SL("Percentage Level Modifier", -100, 155, 0, "%", { key: "lvl", tog: true, free, t: "stpPercentageLevelModifierCheckBox" })
      ], { pfx: ["stp"], tips: ["stpUnchangedRadioButton", "stpSwapLegendariesSwapStandardsRadioButton", "stpRandomCompletelyRadioButton", "stpRandomSimilarStrengthRadioButton"] }),
      G("In-Game Trades", "igt", ["Unchanged", "Randomize Given Pokémon Only", "Randomize Both Requested & Given Pokémon"], 0, [
        T("Randomize Nicknames", false, { key: "nick", t: "igtRandomizeNicknamesCheckBox" }),
        T("Randomize OTs", false, { key: "ot", vis: (r: RomInfo) => r.genNum !== 1, t: "igtRandomizeOTsCheckBox" }),
        T("Randomize IVs", false, { key: "iv", vis: (r: RomInfo) => r.genNum !== 1, t: "igtRandomizeIVsCheckBox" }),
        T("Randomize Items", false, { key: "item", vis: (r: RomInfo) => r.genNum !== 1, t: "igtRandomizeItemsCheckBox" })
      ], { pfx: ["igt"] })
    ]},
    { id: "moves", label: "Moves & Movesets", icon: "M13 2L4 14h7l-1 8 9-12h-7z", desc: "Move data itself, and which moves each Pokémon learns by level.", groups: [
      G("Move Data", "md", null, 0, [
        T("Randomize Move Power", false, { key: "pow", free, t: "mdRandomizeMovePowerCheckBox" }),
        T("Randomize Move Accuracy", false, { key: "acc", free, t: "mdRandomizeMoveAccuracyCheckBox" }),
        T("Randomize Move PP", false, { key: "pp", free, t: "mdRandomizeMovePPCheckBox" }),
        T("Randomize Move Types", false, { key: "type", free, t: "mdRandomizeMoveTypesCheckBox" }),
        T("Randomize Move Category", false, { key: "cat", free, vis: (r: RomInfo) => r.split, t: "mdRandomizeMoveCategoryCheckBox" }),
        T("Randomize Move Names", false, { key: "name", free, vis: (r: RomInfo) => r.english, t: "mdRandomizeMoveNamesCheckBox" }),
        H("Updates"),
        SEL("Update Moves to Generation", "MDGENS", 0, { key: "upd", tog: true, free, vis: (r: RomInfo) => r.genNum < 8, t: "mdUpdateMovesCheckBox" })
      ], { pfx: ["md"] }),
      G("Pokémon Movesets", "pms", ["Unchanged", "Random (preferring same type)", "Random (completely)", "Metronome Only Mode"], 0, [
        SL("Guaranteed Level 1 Moves", 2, 4, 2, "", { key: "l1", tog: true, need: [1, 2], t: "pmsGuaranteedLevel1MovesCheckBox" }),
        T("Reorder Damaging Moves", false, { key: "ord", need: [1, 2], t: "pmsReorderDamagingMovesCheckBox" }),
        T("No Game-Breaking Moves", false, { key: "ngb", need: [1, 2], t: "pmsNoGameBreakingMovesCheckBox" }),
        SL("Force % of Good Damaging Moves", 0, 100, 0, "%", { key: "good", tog: true, need: [1, 2], t: "pmsForceGoodDamagingCheckBox" }),
        T("Evolution Moves for All Pokémon", false, { key: "evo", need: [1, 2], vis: (r: RomInfo) => r.genNum >= 7, t: "pmsEvolutionMovesCheckBox" })
      ], { pfx: ["pms"] })
    ]},
    { id: "foe", label: "Foe Pokémon", icon: "M14.5 17.5L3 6V3h3l11.5 11.5M13 19l6-6M16 16l4 4M19 21l2-2", desc: "Trainer teams, battle formats and totem encounters.", groups: [
      G("Trainer Pokémon", "tp", ["Unchanged", "Random", "Random (even distribution)", "Random (even distribution, main-game)", "Type Themed", "Type Themed (Elite Four/Gyms Only)", "Keep Type Themed Trainers’ Themes", "Keep Themes Or Primary"], 0, [
        H("Selection"),
        T("Rival Carries Starter Through Game", false, { key: "rival", free, en: (c: Ctx) => !c.el() && (c.m("sp") !== 0 || c.m("tp") !== 0), t: "tpRivalCarriesStarterCheckBox" }),
        T("Try to Use Pokémon with Similar Strength", false, { key: "sim", free, en: tpBasic, t: "tpSimilarStrengthCheckBox" }),
        T("Try to Avoid Duplicates", false, { key: "dup", free, en: tpBasic, t: "tpAvoidDuplicatesCheckBox" }),
        T("Weight Types by # of Pokémon", false, { key: "weight", free, en: (c: Ctx) => c.m("tp") === 4, t: "tpWeightTypesCheckBox" }),
        T("Don’t Use Legendaries", false, { key: "noleg", free, en: tpBasic, t: "tpDontUseLegendariesCheckBox" }),
        T("No Early Wonder Guard", false, { key: "wg", free, en: tpBasic, vis: (r: RomInfo) => r.genNum >= 3, t: "tpNoEarlyWonderGuardCheckBox" }),
        T("Use Local Pokémon", false, { key: "local", free, en: tpBasic, t: "tpUseLocalPokemonCheckBox" }),
        T("Allow Alternate Formes", false, { key: "alt", free, en: tpBasic, vis: (r: RomInfo) => r.funcFormes, t: "tpAllowAlternateFormesCheckBox" }),
        T("Swap Mega Evolvables", false, { key: "mega", free, en: (c: Ctx) => c.m("tp") !== 0, vis: (r: RomInfo) => r.mega, t: "tpSwapMegaEvosCheckBox" }),
        T("Random Shiny Trainer Pokémon", false, { key: "shiny", free, en: tpBasic, vis: (r: RomInfo) => r.genNum >= 7, t: "tpRandomShinyTrainerPokemonCheckBox" }),
        H("Names"),
        T("Randomize Trainer Names", false, { key: "names", free, t: "tpRandomizeTrainerNamesCheckBox" }),
        T("Randomize Trainer Class Names", false, { key: "classes", free, t: "tpRandomizeTrainerClassNamesCheckBox" }),
        H("Levels & evolution"),
        SL("Trainers Evolve their Pokémon", -100, 155, 0, "%", { key: "evo", tog: true, free, en: notEL, t: "tpTrainersEvolveTheirPokemonCheckbox" }),
        I("evo"),
        SL("Percentage Level Modifier", -100, 155, 0, "%", { key: "lvl", tog: true, free, t: "tpPercentageLevelModifierCheckBox" }),
        NUM("Pokémon League Has Unique Pokémon", 1, 1, 2, { key: "e4", tog: true, free, en: (c: Ctx) => c.m("tp") !== 0, t: "tpEliteFourUniquePokemonCheckBox" }),
        H("Held items"),
        T("Consumable Only", false, { key: "cons", free, en: (c: Ctx) => c.m("tp") !== 0 && c.tpItems(), t: "tpConsumableItemsOnlyCheckBox" }),
        T("Sensible Items", false, { key: "sens", free, en: (c: Ctx) => c.m("tp") !== 0 && c.tpItems(), t: "tpSensibleItemsCheckBox" }),
        T("Highest Level Only", false, { key: "high", free, en: (c: Ctx) => c.m("tp") !== 0 && c.tpItems(), t: "tpHighestLevelGetsItemCheckBox" }),
        tier(0, 0, { val: 1, num: true, t: "tpBossTrainersCheckBox" }), tier(0, 1, { val: 1, num: true, t: "tpImportantTrainersCheckBox" }), tier(0, 2, { val: 1, num: true, t: "tpRegularTrainersCheckBox" }),
        tier(1, 0, { en: (c: Ctx) => c.m("tp") !== 0 }), tier(1, 1, { en: (c: Ctx) => c.m("tp") !== 0 }), tier(1, 2, { en: (c: Ctx) => c.m("tp") !== 0 }),
        tier(2, 0, { en: (c: Ctx) => c.tpDiv(0) }), tier(2, 1, { en: (c: Ctx) => c.tpDiv(1) }), tier(2, 2, { en: (c: Ctx) => c.tpDiv(2) }),
        tier(3, 0), tier(3, 1), tier(3, 2)
      ], { wide: true, matrix: ["Additional Pokémon", "Add held items", "Force diverse types", "Better movesets"], matrixTips: ["tpBossTrainersCheckBox", "tpBossTrainersItemsCheckBox", "tpBossTrainersTypeDiversityCheckBox", "tpBetterMovesetsBossTrainersCheckBox"], pfx: ["tp"], mvis: (r: RomInfo, i: number) => !(i === 3 && r.genNum !== 5), tips: ["tpMain0Unchanged", "tpMain1Random", "tpMain2RandomEvenDistribution", "tpMain3RandomEvenDistributionMainGame", "tpMain4TypeThemed", "tpMain5TypeThemedEliteFourGyms", "tpMain6KeepThemed", "tpMain7KeepThemed"] }),
      G("Battle Style", "bs", ["Unchanged", "Random", "Single Style"], 0, [
        SEL("Style", "BATTLESTYLES", 0, { key: "style", need: [2] })
      ], { vis: (r: RomInfo) => r.genNum >= 3, tips: ["tpUnchangedBattleStyleRadioButton", "tpRandomBattleStyleRadioButton", "tpSingleStyleBattleStyleRadioButton"] }),
      G("Totem Pokémon", "totp", ["Unchanged", "Random", "Random (similar strength)"], 0, [
        RA("Ally Pokémon", ["Unchanged", "Random", "Random (similar strength)"], 0, { key: "ally", free, tips: ["totpAllyUnchangedRadioButton", "totpAllyRandomRadioButton", "totpAllyRandomSimilarStrengthRadioButton"] }),
        RA("Auras", ["Unchanged", "Random", "Random (same strength)"], 0, { key: "aura", free, tips: ["totpAuraUnchangedRadioButton", "totpAuraRandomRadioButton", "totpAuraRandomSameStrengthRadioButton"] }),
        T("Randomize Held Items", false, { key: "held", free, t: "totpRandomizeHeldItemsCheckBox" }),
        T("Allow Alternate Formes", false, { key: "alt", free, en: (c: Ctx) => c.m("totp") !== 0 || c.v("totp.ally") !== 0, t: "totpAllowAltFormesCheckBox" }),
        SL("Percentage Level Modifier", -100, 155, 0, "%", { key: "lvl", tog: true, free, t: "totpPercentageLevelModifierCheckBox" })
      ], { pfx: ["totp"], vis: (r: RomInfo) => r.totem })
    ]},
    { id: "wild", label: "Wild Pokémon", icon: "M5 21c0-9 6-15 16-16-1 10-7 16-16 16zM5 21l8-8", desc: "What you meet in grass, caves, water and every other encounter table.", groups: [
      G("Wild Pokémon", "wp", ["Unchanged", "Randomize Wild Pokémon"], 0, [
        RA("Zones", ["Maximum Possible", "1 Per Encounter Set", "1 Per Map", "1 Per Named Location", "1 In Whole Game"], 4, { key: "zone", need: [1], def: 4, cvis: (r: RomInfo, ci: number) => (ci === 1 ? !r.mapIdx : ci === 2 ? r.mapIdx : ci === 3 ? r.locs : true), tips: ["wpZoneNoneRadioButton", "wpZoneEncounterSetRadioButton", "wpZoneMapRadioButton", "wpZoneLocationRadioButton", "wpZoneGlobalRadioButton"] }),
        T("Split by Encounter Types", false, { key: "split", need: [1], en: (c: Ctx) => c.v("wp.zone") !== 0 && c.v("wp.zone") !== 1, t: "wpSplitByEncounterTypesCheckBox" }),
        RA("Type restrictions", ["None", "Random Zone Themes", "Keep Primary Type"], 0, { key: "tr", need: [1], cen: (c: Ctx, ci: number) => !(ci === 1 && c.v("wp.zone") === 4), tips: ["wpTRNoneRadioButton", "wpTRThemedAreasRadioButton", "wpTRKeepPrimaryRadioButton"] }),
        T("Keep Set/Zone Themes", false, { key: "themes", need: [1], t: "wpTRKeepThemesCheckBox" }),
        RA("Evolution restrictions", ["None", "Only Basic Pokémon", "Same Evolution Stage"], 0, { key: "er", need: [1], en: notEL, tips: ["wpERNoneRadioButton", "wpERBasicOnlyRadioButton", "wpERSameEvolutionStageRadioButton"] }),
        T("Keep Relations", false, { key: "rel", need: [1], en: (c: Ctx) => c.v("wp.zone") !== 0, t: "wpERKeepEvolutionsCheckBox" }),
        H("Encounter options"),
        T("Similar Strength", false, { key: "sim", need: [1], t: "wpSimilarStrengthCheckBox" }),
        T("Balance Low Level Encounters", false, { key: "bal", free, en: (c: Ctx) => c.on("wp.sim"), t: "wpBalanceShakingGrassPokemonCheckBox" }),
        T("Catch Em All Mode", false, { key: "cea", need: [1], en: (c: Ctx) => c.v("wp.zone") !== 4 || c.on("wp.split"), t: "wpCatchEmAllModeCheckBox" }),
        T("Use Time Based Encounters", true, { key: "time", need: [1], disOn: true, vis: (r: RomInfo) => r.timeEnc, t: "wpUseTimeBasedEncountersCheckBox" }),
        T("Don’t Use Legendaries", false, { key: "noleg", need: [1], t: "wpDontUseLegendariesCheckBox" }),
        T("Allow Alternate Formes", false, { key: "alt", need: [1], vis: (r: RomInfo) => r.wildAlt, t: "wpAllowAltFormesCheckBox" }),
        H("Always available"),
        SL("Set Minimum Catch Rate", 1, 5, 1, "", { key: "catch", tog: true, free, t: "wpSetMinimumCatchRateCheckBox" }),
        T("Randomize Held Items", false, { key: "held", free, vis: (r: RomInfo) => r.genNum !== 1, t: "wpRandomizeHeldItemsCheckBox" }),
        T("Ban Bad Items", false, { key: "ban", free, vis: (r: RomInfo) => r.genNum !== 1, en: (c: Ctx) => c.on("wp.held"), t: "wpBanBadItemsCheckBox" }),
        SL("Percentage Level Modifier", -100, 155, 0, "%", { key: "lvl", tog: true, free, t: "wpPercentageLevelModifierCheckBox" })
      ], { wide: true, pfx: ["wp"], tips: ["", "wpRandomizeWildPokemonCheckBox"] })
    ]},
    { id: "tms", label: "TMs & Tutors", icon: "M12 3a9 9 0 1 0 0 18 9 9 0 1 0 0-18zM12 9a3 3 0 1 0 0 6 3 3 0 1 0 0-6z", desc: "Which moves TMs, HMs and tutors teach, and who can learn them.", groups: [
      G("TM Moves", "tm", ["Unchanged", "Random"], 0, [
        T("Keep Field Move TMs", false, { key: "field", t: "tmKeepFieldMoveTMsCheckBox" }),
        T("No Game-Breaking Moves", false, { key: "ngb", t: "tmNoGameBreakingMovesCheckBox" }),
        SL("Force % of Good Damaging Moves", 0, 100, 0, "%", { key: "good", tog: true, t: "tmForceGoodDamagingCheckBox" })
      ], { pfx: ["tm"], men: (c: Ctx, i: number) => !(i === 1 && c.m("pms") === 3), mfall: 0 }),
      G("TM/HM Compatibility", "thc", compat, 0, [
        T("TM/Levelup Move Sanity", false, { key: "san", free, en: (c: Ctx) => c.m("pms") !== 3 && !(c.m("pms") === 0 && c.m("tm") === 0 && c.m("thc") === 0), t: "tmLevelupMoveSanityCheckBox" }),
        T("Follow Evolutions", false, { key: "fe", free, en: (c: Ctx) => c.m("pms") !== 3 && !c.el() && (c.m("thc") !== 0 || c.on("thc.san")), t: "tmFollowEvolutionsCheckBox" }),
        T("Full HM Compatibility", false, { key: "hm", free, vis: (r: RomInfo) => r.genNum < 7, en: (c: Ctx) => c.m("thc") !== 3, t: "tmFullHMCompatibilityCheckBox" })
      ], { pfx: ["thc"] }),
      G("Move Tutor Moves", "mt", ["Unchanged", "Random"], 0, [
        T("Keep Field Move Tutors", false, { key: "field", t: "mtKeepFieldMoveTutorsCheckBox" }),
        T("No Game-Breaking Moves", false, { key: "ngb", t: "mtNoGameBreakingMovesCheckBox" }),
        SL("Force % of Good Damaging Moves", 0, 100, 0, "%", { key: "good", tog: true, t: "mtForceGoodDamagingCheckBox" })
      ], { pfx: ["mt"], men: (c: Ctx, i: number) => !(i === 1 && c.m("pms") === 3), mfall: 0, na: (r: RomInfo) => (r.tutors ? "" : "This game does not have any Move Tutors, or they are not randomizable yet.") }),
      G("Move Tutor Compatibility", "mtc", compat, 0, [
        T("Tutor/Levelup Move Sanity", false, { key: "san", free, en: (c: Ctx) => c.m("pms") !== 3 && !(c.m("pms") === 0 && c.m("mt") === 0 && c.m("mtc") === 0), t: "mtLevelupMoveSanityCheckBox" }),
        T("Follow Evolutions", false, { key: "fe", free, en: (c: Ctx) => c.m("pms") !== 3 && !c.el() && (c.m("mtc") !== 0 || c.on("mtc.san")), t: "mtFollowEvolutionsCheckBox" })
      ], { pfx: ["mtc"], vis: (r: RomInfo) => r.tutors })
    ]},
    { id: "items", label: "Items", icon: "M6 8h12l-1 12H7zM9 8V6a3 3 0 0 1 6 0v2", desc: "Items on the ground, in shops and from Pickup.", groups: [
      G("Field Items", "fi", ["Unchanged", "Shuffle", "Random", "Random (even distribution)"], 0, [
        T("Ban Bad Items", false, { key: "ban", need: [2, 3], t: "fiBanBadItemsCheckBox" })
      ], { pfx: ["fi"] }),
      G("Shop Items", "sh", ["Unchanged", "Shuffle", "Random"], 0, [
        H("Special shop items"),
        T("Ban Overpowered Shop Items", false, { key: "op", need: [2], t: "shBanOverpoweredShopItemsCheckBox" }),
        T("Ban Bad Items", false, { key: "bad", need: [2], t: "shBanBadItemsCheckBox" }),
        T("Ban Regular Shop Items", false, { key: "reg", need: [2], t: "shBanRegularShopItemsCheckBox" }),
        T("Guarantee Evolution Items", false, { key: "evo", need: [2], t: "shGuaranteeEvolutionItemsCheckBox" }),
        T("Guarantee X Items", false, { key: "x", need: [2], t: "shGuaranteeXItemsCheckBox" }),
        H("All shops"),
        T("Balance Shop Item Prices", false, { key: "price", free, t: "shBalanceShopItemPricesCheckBox" }),
        T("Add Cheap Rare Candies", false, { key: "candy", free, vis: (r: RomInfo) => r.shopSizes, t: "shAddRareCandyCheckBox" })
      ], { pfx: ["sh"] }),
      G("Pickup Items", "pu", ["Unchanged", "Random"], 0, [
        T("Ban Bad Items", false, { key: "ban", need: [1], t: "puBanBadItemsCheckBox" })
      ], { pfx: ["pu"], vis: (r: RomInfo) => r.genNum >= 3 })
    ]},
    { id: "types", label: "Types", icon: "M12 2l3 7h7l-5.5 4.5 2 7.5L12 16.5 5.5 21l2-7.5L2 9h7z", desc: "The type chart: which types are strong, weak or immune against each other.", groups: [
      G("Type Effectiveness", "te", ["Unchanged", "Random", "Random (balanced)", "Keep Type Identities", "Inverse"], 0, [
        T("Add Random Immunities", false, { key: "imm", need: [4], t: "teAddRandomImmunitiesCheckBox" }),
        T("Update Type Effectiveness", false, { key: "upd", free, vis: (r: RomInfo) => r.genNum < 6, t: "teUpdateTypeEffectivenessCheckBox" })
      ], { pfx: ["te"] })
    ]},
    { id: "graphics", label: "Graphics", icon: "M12 3a9 9 0 0 0 0 18c1.1 0 1.5-.8 1.5-1.6 0-1.4 1-2.4 2.4-2.4H18a3 3 0 0 0 3-3c0-6-4-11-9-11zM7.5 11h.01M10 7h.01M15 7h.01", desc: "Pokémon colour palettes and custom player sprites.", unsupported: (r: RomInfo) => !r.palette && !r.cpg, groups: [
      G("Pokémon Palettes", "ppal", ["Unchanged", "Random"], 0, [
        T("Follow Types", false, { key: "types", t: "ppalFollowTypesCheckBox" }),
        T("Follow Evolutions", false, { key: "evo", t: "ppalFollowEvolutionsCheckBox" }),
        T("Shiny From Normal", false, { key: "shiny", vis: (r: RomInfo) => r.genNum !== 1, t: "ppalShinyFromNormalCheckBox" })
      ], { pfx: ["ppal"], na: (r: RomInfo) => (r.palette ? "" : "This game does not have any Pokémon palettes, or they are not randomizable yet."), note: (r: RomInfo) => (r.palPartial ? "This game supports palette randomization, but not every palette yet. Even with the options below, only some palettes will change." : "") }),
      G("Custom Player Graphics", "cpg", ["Unchanged", "Custom"], 0, [
        RA("Character to replace", ["Player character 1", "Player character 2"], 0, { key: "who", need: [1], cen: (c: Ctx, ci: number) => ci === 0 || c.r.cpgPc2, tips: ["cpgReplaceRadioButton1", "cpgReplaceRadioButton2"] })
      ], { wide: true, cpg: true, pfx: ["cpg"], na: (r: RomInfo) => (r.cpg ? "" : "This game does not support custom player graphics (yet).") })
    ]},
    { id: "misc", label: "Misc. Tweaks", icon: "M14.7 6.3a4 4 0 0 0-5.4 5.4L3 18l3 3 6.3-6.3a4 4 0 0 0 5.4-5.4l-2.6 2.6-2.4-.6-.6-2.4z", desc: "Quality-of-life code patches. Only the tweaks this game supports are shown.", groups: [
      G("Code Tweaks", "tw", null, 0, [], { wide: true, tweaks: true })
    ]}
  ];
}
