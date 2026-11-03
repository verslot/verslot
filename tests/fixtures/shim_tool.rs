use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

fn main() {
    let executable = std::env::current_exe().unwrap();
    let version = executable
        .ancestors()
        .filter_map(Path::file_name)
        .filter_map(|name| name.to_str())
        .find(|name| name.matches('.').count() == 2)
        .unwrap_or("system");
    let mut arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.first().is_some_and(|argument| {
        Path::new(argument)
            .extension()
            .is_some_and(|extension| extension == "js")
    }) {
        println!(
            "bundled={};node={version}",
            fs::read_to_string(&arguments.remove(0)).unwrap()
        );
    }
    match arguments
        .first()
        .and_then(|argument| argument.to_str())
        .unwrap_or("")
    {
        "--version" => println!("v{version}"),
        "inspect" => {
            println!("version={version}");
            println!(
                "cwd={:?}",
                fs::canonicalize(std::env::current_dir().unwrap()).unwrap()
            );
            println!("env={}", std::env::var("VERSLOT_FIXTURE_VALUE").unwrap());
            println!("args={:?}", &arguments[1..]);
            let mut input = Vec::new();
            io::stdin().read_to_end(&mut input).unwrap();
            io::stdout().write_all(&input).unwrap();
            eprintln!("tool stderr");
            std::process::exit(37);
        }
        "exit" => std::process::exit(arguments[1].to_str().unwrap().parse().unwrap()),
        "block" => {
            fs::write(&arguments[1], b"ready").unwrap();
            let mut input = String::new();
            io::stdin().read_to_string(&mut input).unwrap();
            println!("v{version}:{input}");
        }
        #[cfg(unix)]
        "interrupt" => {
            use std::os::unix::process::CommandExt;
            let error = std::process::Command::new("/bin/sh").arg("-c").arg(
                "trap 'printf interrupted > \"$2\"; while [ ! -e \"$3\" ]; do sleep 0.01; done; exit 37' INT; printf ready > \"$1\"; while :; do sleep 0.01; done"
            ).arg("fixture").args(&arguments[1..]).exec();
            panic!("{error}");
        }
        _ => {}
    }
}
