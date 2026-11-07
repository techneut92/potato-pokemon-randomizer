//! Custom player graphics: the chosen graphics pack goes into the game, last. Nothing is random; the ROM handler writes
//! the pack.

use ppr_model::PlayerGraphicsChoice;

use crate::{RandomizeError, RunData, Step, StepContext, step_ids};

const LABEL: &str = "Custom Player Graphics";

/// Puts [`ppr_settings::PlayerGraphicsSettings`] into the game data.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlayerGraphicsChooser;

impl Step<RunData> for PlayerGraphicsChooser {
    fn id(&self) -> u64 {
        step_ids::PLAYER_GRAPHICS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.player_graphics.pack.is_some()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let data = ctx.data;
        let settings = &data.settings.player_graphics;
        let Some(pack) = &settings.pack else {
            return Ok(());
        };
        if !data.game.player_graphics_packs.iter().any(|p| &p.id == pack) {
            return Err(RandomizeError::Impossible {
                step: LABEL,
                message: format!("this game has no player graphics pack \"{pack}\""),
            });
        }
        data.game.player_graphics = Some(PlayerGraphicsChoice {
            pack: pack.clone(),
            replace: settings.replace,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::{PlayerCharacter, PlayerGraphicsPack, Type};
    use ppr_settings::{PlayerGraphicsSettings, Settings};

    use super::*;
    use crate::randomizers::testing::{emerald, species};
    use crate::{CancelToken, Progress, Seed, standard_pipeline};

    fn run(pack: Option<&str>) -> Result<RunData, RandomizeError> {
        let mut game = emerald(vec![species(1, Type::Grass, None, 45)]);
        game.player_graphics_packs = vec![PlayerGraphicsPack {
            id: "leaf".into(),
            name: "Leaf".into(),
            description: String::new(),
            from: String::new(),
            creator: String::new(),
            adapter: None,
            category: None,
            rom_type: "RSE".into(),
        }];
        let mut data = RunData {
            game,
            settings: Settings {
                player_graphics: PlayerGraphicsSettings {
                    pack: pack.map(str::to_owned),
                    replace: PlayerCharacter::Pc2,
                },
                ..Settings::default()
            },
        };
        standard_pipeline().run(&mut data, Seed(1), &CancelToken::default(), &mut |_: Progress| {})?;
        Ok(data)
    }

    #[test]
    fn the_chosen_pack_goes_into_the_game() {
        let data = run(Some("leaf")).expect("runs");
        assert_eq!(
            data.game.player_graphics,
            Some(PlayerGraphicsChoice {
                pack: "leaf".into(),
                replace: PlayerCharacter::Pc2,
            })
        );
        assert_eq!(run(None).expect("runs").game.player_graphics, None);
        assert!(run(Some("missing")).is_err());
    }
}
