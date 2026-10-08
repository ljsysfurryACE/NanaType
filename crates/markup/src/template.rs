/// Replace `{{key}}` placeholders. Unknown keys stay as typed so a template
/// never silently loses text; `{{{{` is a literal `{{`.
pub fn expand_template(template: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some(stripped) = after.strip_prefix("{{") {
            out.push_str("{{");
            rest = stripped;
            continue;
        }
        match after.find("}}") {
            Some(end) => {
                let key = after[..end].trim();
                match lookup(key) {
                    Some(value) => out.push_str(&value),
                    None => out.push_str(&rest[start..start + 2 + end + 2]),
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}
