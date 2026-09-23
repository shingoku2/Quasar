## 2023-10-27 - Preventing Command/Option Injection in ssh invocation
**Vulnerability:** Invoking `ssh` externally using `std::process::Command` without using `--` to separate options from targets can allow users to inject arbitrary options (like `-o ProxyCommand="..."`) into the SSH invocation if the target string (e.g. username or hostname) starts with a hyphen.
**Learning:** Always use `--` argument separators when constructing external commands with user-provided arguments, even if the arguments are validated, as an extra layer of defense (Defense in Depth). Also, strictly validate that user inputs don't start with hyphens where applicable.
**Prevention:** Use `--` before the target argument when spawning the SSH process to prevent it from being parsed as an option.
## 2025-03-01 - Avoid Returning Decrypted Passwords to Frontend State
**Vulnerability:** The backend exposed decrypted credential passwords in the `CredentialFrontendView` model returned to the frontend.
**Learning:** Sending decrypted passwords as part of general models stores sensitive data in the React state/DOM, making them susceptible to exposure.
**Prevention:** Avoid returning decrypted secrets in general frontend models. Use narrowly scoped backend commands (like `reveal_credential_password`) to fetch the decrypted secret only when required for explicit display or copying.
