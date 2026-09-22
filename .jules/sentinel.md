## 2023-10-27 - Preventing Command/Option Injection in ssh invocation
**Vulnerability:** Invoking `ssh` externally using `std::process::Command` without using `--` to separate options from targets can allow users to inject arbitrary options (like `-o ProxyCommand="..."`) into the SSH invocation if the target string (e.g. username or hostname) starts with a hyphen.
**Learning:** Always use `--` argument separators when constructing external commands with user-provided arguments, even if the arguments are validated, as an extra layer of defense (Defense in Depth). Also, strictly validate that user inputs don't start with hyphens where applicable.
**Prevention:** Use `--` before the target argument when spawning the SSH process to prevent it from being parsed as an option.
