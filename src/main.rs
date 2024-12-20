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
        //         Executor::Piston(
        //             "python".to_string(),
        //             r#"
        // import json, sys
        // print(json.dumps({"action": True, "storage": json.loads(sys.argv[2])}))
        // "#
        //             .to_string(),
        //         ),
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
    let forgiving_tit_for_tat = Player::new(
        "Forgiving Tit for Tat".to_string(),
        Executor::Lua(
            r#"
            function(history, storage)
                if #history == 0 then
                    return true, storage
                elseif #history == 1 then
                    return history[#history][2], storage
                else
                    if not history[#history][2] and not history[#history - 1][2] then
                        return false, storage
                    else
                        return true, storage
                    end
                end
            end
            "#
            .to_string(),
        ),
    );

    let detective = Player::new(
        "Detective".to_string(),
        Executor::Python(
            r#"
def main(history, storage):
    if len(history) < 4:
        moves = [True, False, True, True]
        return moves[len(history)], storage
    else:
        for round in history:
            if not round[1]:
                return history[-1][1], storage
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
                forgiving_tit_for_tat,
                detective,
            ])
            .with_rounds(200),
    );

    let scores = tournament.run();
    for (i, player) in tournament.config.players.iter().enumerate() {
        println!("{}: {}", player.name, scores[i]);
    }
}
