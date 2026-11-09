//! The start screen's game lists. They mirror the web UI; tests keep them equal to it and to the ROM catalogue.

/// The file types the drop zone names (the chips under its buttons).
pub(crate) const EXTENSIONS: [&str; 7] = [".gb", ".gbc", ".gba", ".nds", ".3ds", ".cia", ".cxi"];

/// One card of "Supported games": a generation, its console as players know it, and its games.
pub(crate) struct GenerationCard {
    pub(crate) generation: u8,
    pub(crate) console: &'static str,
    pub(crate) games: &'static [&'static str],
}

impl GenerationCard {
    pub(crate) fn title(&self) -> String {
        format!("Generation {}", self.generation)
    }
}

/// `SUPPORTED_GAMES` in `src/lib/data/roms.ts`.
pub(crate) const SUPPORTED_GAMES: [GenerationCard; 7] = [
    GenerationCard {
        generation: 1,
        console: "Game Boy",
        games: &["Red", "Blue", "Green (JP)", "Yellow"],
    },
    GenerationCard {
        generation: 2,
        console: "Game Boy Color",
        games: &["Gold", "Silver", "Crystal"],
    },
    GenerationCard {
        generation: 3,
        console: "Game Boy Advance",
        games: &["Ruby", "Sapphire", "Emerald", "FireRed", "LeafGreen"],
    },
    GenerationCard {
        generation: 4,
        console: "Nintendo DS",
        games: &["Diamond", "Pearl", "Platinum", "HeartGold", "SoulSilver"],
    },
    GenerationCard {
        generation: 5,
        console: "Nintendo DS",
        games: &["Black", "White", "Black 2", "White 2"],
    },
    GenerationCard {
        generation: 6,
        console: "Nintendo 3DS",
        games: &["X", "Y", "Omega Ruby", "Alpha Sapphire"],
    },
    GenerationCard {
        generation: 7,
        console: "Nintendo 3DS",
        games: &["Sun", "Moon", "Ultra Sun", "Ultra Moon"],
    },
];

#[cfg(test)]
mod tests {
    use ppr_model::Platform;

    use super::*;

    const ROMS_TS: &str = include_str!("../../src/lib/data/roms.ts");
    const WELCOME: &str = include_str!("../../src/lib/components/Welcome.svelte");

    /// The quoted strings of `line`, in order.
    fn quoted(line: &str) -> Vec<&str> {
        line.split('\'').skip(1).step_by(2).collect()
    }

    #[test]
    fn supported_games_match_the_web_ui() {
        let start = ROMS_TS
            .find("export const SUPPORTED_GAMES")
            .expect("SUPPORTED_GAMES is in roms.ts");
        let rows: Vec<Vec<&str>> = ROMS_TS[start..]
            .lines()
            .skip(1)
            .take_while(|line| !line.starts_with("];"))
            .map(quoted)
            .collect();
        assert_eq!(rows.len(), SUPPORTED_GAMES.len());
        for (row, card) in rows.iter().zip(&SUPPORTED_GAMES) {
            let mut ours = vec![card.title()];
            ours.push(card.console.to_owned());
            ours.extend(card.games.iter().map(|g| (*g).to_owned()));
            assert_eq!(row, &ours, "{} differs from roms.ts", card.title());
        }
    }

    #[test]
    fn extensions_match_the_web_ui() {
        let line = WELCOME
            .lines()
            .find(|l| l.contains("const EXTENSIONS"))
            .expect("EXTENSIONS is in Welcome.svelte");
        assert_eq!(quoted(line), EXTENSIONS);
    }

    /// `"Fire Red (U) 1.0"` → `"firered"`, `"Green (JP)"` → `"green"`.
    fn title_key(name: &str) -> String {
        let base = name.split(" (").next().unwrap_or(name);
        base.chars()
            .filter(|c| !c.is_whitespace())
            .flat_map(char::to_lowercase)
            .collect()
    }

    #[test]
    fn every_listed_game_is_in_the_rom_catalogue() {
        let catalogue = ppr_service::supported_games();
        for card in &SUPPORTED_GAMES {
            for game in card.games {
                let key = title_key(game);
                assert!(
                    catalogue
                        .iter()
                        .any(|e| e.generation.number() == card.generation && title_key(&e.name) == key),
                    "{game} (Gen {}) isn't in the ROM catalogue",
                    card.generation
                );
            }
        }
    }

    #[test]
    fn consoles_match_the_model() {
        for card in &SUPPORTED_GAMES {
            let generation = ppr_model::Generation::new(card.generation).expect("a supported generation");
            // The model has one platform for Game Boy and Game Boy Color; the card names the Color for Gen 2.
            let expected = match generation.platform() {
                Platform::GameBoy if card.generation == 2 => "Game Boy Color",
                platform => platform.display_name(),
            };
            assert_eq!(card.console, expected, "Gen {}", card.generation);
        }
    }
}
