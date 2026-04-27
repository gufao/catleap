pub mod bundled;
pub mod prefix;
pub mod runner;

use std::path::Path;
use std::process::Command;

/// Build a `Command` that invokes `wine_binary` under Rosetta (`arch -x86_64`).
/// Apple GPTK Wine is x86_64-only and must be launched this way on Apple Silicon.
pub(crate) fn wine_command(wine_binary: &Path) -> Command {
    let mut cmd = Command::new("/usr/bin/arch");
    cmd.arg("-x86_64");
    cmd.arg(wine_binary);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    #[test]
    fn wine_command_uses_arch_x86_64() {
        let cmd = wine_command(&PathBuf::from("/tmp/wine64"));
        assert_eq!(cmd.get_program(), "/usr/bin/arch");
        let args: Vec<&OsStr> = cmd.get_args().collect();
        assert_eq!(args.len(), 2, "wine_command should pass exactly 2 args to arch");
        assert_eq!(args[0], "-x86_64");
        assert_eq!(args[1], "/tmp/wine64");
    }
}
