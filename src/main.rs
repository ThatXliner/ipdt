use ipdt::core::Executor;
use ipdt::core::Player;
use ipdt::core::Tournament;
use ipdt::core::TournamentConfig;
fn main() {
    let tit_for_tat = Player::new(
        "Tit for Tat".to_string(),
        Executor::Lua(
            r#"
            function(history, storage)
                if #history == 0 then
                    return true, storage
                else
                    return history[#history][2], storage
                end
            end
            "#
            .to_string(),
        ),
    );

    let grim_trigger = Player::new(
        "Grim Trigger".to_string(),
        Executor::Lua(
            r#"
            function(history, storage)
                if storage == "defect" then
                    return false, storage
                end
                for _, round in ipairs(history) do
                    if not round[2] then
                        return false, "defect"
                    end
                end
                return true, storage
            end
            "#
            .to_string(),
        ),
    );

    let always_cooperate = Player::new(
        "Always Cooperate".to_string(),
        Executor::Lua(
            r#"
            function(history, storage)
                return true, storage
            end
            "#
            .to_string(),
        ),
    );

    let always_defect = Player::new(
        "Always Defect".to_string(),
        // Executor::Lua(
        //     r#"
        //     function(history, storage)
        //         return false, storage
        //     end
        //     "#
        //     .to_string(),
        // ),
        Executor::Python(
            r#"
def main(history, storage):
    return False, storage
"#
            .to_string(),
        ),
    );

    let mut tournament = Tournament::with_config(
        TournamentConfig::new()
            .with_players(vec![
                tit_for_tat,
                grim_trigger,
                always_cooperate,
                always_defect,
            ])
            .with_rounds(100),
    );

    let scores = tournament.run();
    for (i, player) in tournament.config.players.iter().enumerate() {
        println!("{}: {}", player.name, scores[i]);
    }
}
