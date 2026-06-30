use shlex::Shlex;

pub fn split_command(command: &str) -> Vec<String> {
    let mut arguments = Vec::new();
    Shlex::new(command).for_each(|v| arguments.push(v));

    arguments
}
