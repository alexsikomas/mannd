use std::process::Command;

fn get_networkd() {
    let mut ls = Command::new("ls");
    ls.arg("/etc/systemd/network/");
    match str::from_utf8(&ls.output().unwrap().stdout) {
        Ok(val) => {
            println!("{:?}", val);
        }
        Err(_) => panic!("Could not read from /etc/systemd/network/"),
    }
}
