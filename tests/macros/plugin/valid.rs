use discourse::plugin::*;

#[derive(Debug)]
struct TestPlugin;

impl discourse::question::Plugin for TestPlugin {
    fn ask(
        &mut self,
        _message: String,
        _answers: &Answers,
        _stdout: &mut dyn Backend,
        _events: &mut dyn Iterator<Item = discourse::Result<KeyEvent>>,
    ) -> discourse::Result<Answer> {
        Ok(Answer::Int(0))
    }
}

fn main() {
    discourse::questions![Plugin {
        name: "name",
        plugin: TestPlugin,
    }];
}
