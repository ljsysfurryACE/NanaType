use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::literal;

pub const MANIFEST: &str = ".nanatype-pack.json";
pub const MAX_DEFINITION: u64 = 2 * 1024 * 1024;
pub const MAX_STICKERS: usize = 2048;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Definition {
    pub title: String,
    pub author: String,
    pub license: String,
    pub repository: Option<String>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub lib: String,
    pub name: String,
    pub title: String,
    pub translations: BTreeMap<String, String>,
    pub pack_title: String,
    pub path: PathBuf,
    pub height_em: f32,
}

pub fn text(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut out = String::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_DEFINITION + 1)
        .read_to_string(&mut out)
        .map_err(|e| e.to_string())?;
    if out.len() as u64 > MAX_DEFINITION {
        return Err("表情定义超过 2 MiB / Definition exceeds 2 MiB".into());
    }
    Ok(out)
}

/// Relative paths only, on every platform (including Windows separators).
pub fn relative(value: &str) -> Result<PathBuf, String> {
    let value = value.replace('\\', "/");
    if value.is_empty() || value.contains(':') || value.contains('\0') || value.starts_with('/') {
        return Err(format!("Invalid resource path: {value}"));
    }
    let path = PathBuf::from(&value);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format!("Resource path escapes folder: {value}"));
    }
    Ok(path)
}

pub fn contained(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err("Resource is outside sticker folder".into());
    }
    Ok(path)
}

fn label(value: &Value, fallback: &str) -> String {
    value
        .as_str()
        .or_else(|| value["text"].as_str())
        .unwrap_or(fallback)
        .into()
}

fn names(value: &Value) -> String {
    if let Some(a) = value.as_array() {
        return a
            .iter()
            .map(names)
            .filter(|n| !n.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
    }
    value
        .as_str()
        .or_else(|| value["name"].as_str())
        .unwrap_or("")
        .into()
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && !value.chars().any(|c| {
            c.is_whitespace() || matches!(c, '/' | '\\' | ':' | '[' | ']' | '=' | '"' | '\'')
        })
}

pub fn echo(
    source: &str,
    extension: bool,
    language: Option<&Value>,
    only_lib: Option<&str>,
) -> Result<Definition, String> {
    let data = literal::read(source, if extension { '{' } else { '[' })?;
    let mut result = Definition {
        title: String::new(),
        author: String::new(),
        license: String::new(),
        repository: None,
        entries: Vec::new(),
    };
    let mut registries = Vec::new();
    if extension {
        let hooks = data["register_hook"]["loaded"]
            .as_array()
            .ok_or("Missing Echo-Live register_hook.loaded")?;
        for hook in hooks {
            if hook["registry"] == "echolive:emoji" {
                registries.extend(
                    hook["value"]
                        .as_object()
                        .ok_or("Invalid emoji registry")?
                        .values(),
                );
            }
        }
        result.title = label(&data["meta"]["title"], "Echo-Live");
        result.repository = data["meta"]["project_url"].as_str().map(str::to_owned);
    } else {
        registries.extend(data.as_array().ok_or("Invalid Echo-Live emoji list")?);
    }
    let mut seen = HashSet::new();
    for pack in registries {
        if pack["image"]["is_emoji"] == true {
            continue;
        }
        let lib = pack["meta"]["namespace"]
            .as_str()
            .ok_or("Missing emoji namespace")?;
        if only_lib.is_some_and(|id| id != lib) {
            continue;
        }
        let items = pack["content"].as_array().ok_or("Missing emoji content")?;
        if !items.iter().any(|i| i.get("path").is_some()) {
            continue;
        }
        if !valid_id(lib) {
            return Err(format!("Invalid emoji namespace: {lib}"));
        }
        let pack_title = label(&pack["meta"]["title"], lib);
        if result.title.is_empty() {
            result.title = pack_title.clone();
        }
        let author = names(&pack["meta"]["author"]);
        let license = names(&pack["meta"]["license"]);
        if !author.is_empty() && !result.author.contains(&author) {
            result.author = [result.author, author]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
        }
        if !license.is_empty() && !result.license.contains(&license) {
            result.license = [result.license, license]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
        }
        let images = pack["path"]["images"]
            .as_str()
            .ok_or("Missing image base path")?;
        // Extension source archives contain emoji/ directly; Echo-Live's
        // installed path starts with extensions/<extension meta.name>/.
        let prefix = if extension {
            let id = data["meta"]["name"]
                .as_str()
                .ok_or("Missing extension name")?;
            images
                .strip_prefix(&format!("extensions/{id}/"))
                .unwrap_or(images)
        } else {
            images
        };
        let prefix = prefix.trim_end_matches('/');
        let base = if prefix.is_empty() {
            PathBuf::new()
        } else {
            relative(prefix)?
        };
        let height_em = pack["image"]["size"]["height"]["value"]
            .as_str()
            .and_then(|s| s.strip_suffix("em"))
            .and_then(|s| s.parse::<f32>().ok())
            .filter(|h| h.is_finite() && *h > 0.0 && *h <= 32.0)
            .unwrap_or(1.6);
        let translate = pack["path"]["translate"].as_str().unwrap_or("");
        for item in items {
            if item["type"] == "group" || item.get("path").is_none() {
                continue;
            }
            let name = item["name"].as_str().ok_or("Missing emoji name")?;
            if !valid_id(name) || !seen.insert((lib.to_owned(), name.to_owned())) {
                return Err(format!("Invalid or duplicate emoji: {lib}/{name}"));
            }
            let path = base.join(relative(
                item["path"].as_str().ok_or("Invalid emoji path")?,
            )?);
            if !crate::assets::is_image(&path) {
                return Err(format!("Unsupported image: {}", path.display()));
            }
            let mut translations = BTreeMap::new();
            let key = item["title"]["translate"].as_str().unwrap_or(name);
            let keys = [
                format!("emoji.{translate}emoji.{key}"),
                format!("emoji.{translate}{key}"),
            ];
            let patch = &data["localization_patch"]["zho-Hans"];
            for source in [Some(patch), language, language.map(|v| &v["localization"])]
                .into_iter()
                .flatten()
            {
                for key in &keys {
                    let mut value = source;
                    for part in key.split('.') {
                        value = &value[part];
                    }
                    if let Some(v) = value.as_str() {
                        translations.insert("zh-CN".into(), v.into());
                        break;
                    }
                }
            }
            result.entries.push(Entry {
                lib: lib.into(),
                name: name.into(),
                title: label(&item["title"], name),
                translations,
                pack_title: pack_title.clone(),
                path,
                height_em,
            });
        }
    }
    if result.entries.is_empty() {
        return Err("No image stickers found in Echo-Live definition".into());
    }
    if result.entries.len() > MAX_STICKERS {
        return Err("Too many stickers (maximum 2048)".into());
    }
    Ok(result)
}

pub fn load(root: &Path) -> Result<Definition, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if root.join(MANIFEST).is_file() {
        let mut d: Definition =
            serde_json::from_str(&text(&contained(&root, &root.join(MANIFEST))?)?)
                .map_err(|e| e.to_string())?;
        if d.entries.is_empty() || d.entries.len() > MAX_STICKERS {
            return Err("Invalid sticker manifest".into());
        }
        for e in &mut d.entries {
            e.path = relative(e.path.to_str().ok_or("Invalid path encoding")?)?;
            if !valid_id(&e.lib)
                || !valid_id(&e.name)
                || !e.height_em.is_finite()
                || e.height_em <= 0.0
                || e.height_em > 32.0
            {
                return Err("Invalid sticker manifest entry".into());
            }
        }
        return Ok(d);
    }
    let static_path = root.join("res/data/emoji.js");
    let extension_path = root.join("main.js");
    if static_path.is_file() || extension_path.is_file() {
        let extension = !static_path.is_file();
        let path = if extension {
            extension_path
        } else {
            static_path
        };
        let lang_path = root.join("lang/zho-Hans.js");
        let language = if lang_path.is_file() {
            Some(literal::read(&text(&contained(&root, &lang_path)?)?, '{')?)
        } else {
            None
        };
        return echo(
            &text(&contained(&root, &path)?)?,
            extension,
            language.as_ref(),
            None,
        );
    }
    let mut lib = crate::assets::AssetLibrary::new(PathBuf::new());
    lib.scan(&[root.to_string_lossy().into_owned()]);
    let title = root
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut entries = Vec::new();
    for s in lib.stickers {
        contained(&root, &s.path)?;
        entries.push(Entry {
            path: s
                .path
                .strip_prefix(&root)
                .map_err(|e| e.to_string())?
                .into(),
            lib: s.lib,
            title: s.name.clone(),
            name: s.name,
            translations: BTreeMap::new(),
            pack_title: title.clone(),
            height_em: 1.6,
        });
        if entries.len() > MAX_STICKERS {
            return Err("Too many stickers (maximum 2048)".into());
        }
    }
    if entries.is_empty() {
        return Err("No supported sticker images in folder".into());
    }
    Ok(Definition {
        title,
        author: String::new(),
        license: String::new(),
        repository: None,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_are_portable_and_confined() {
        for p in [
            "../x.png",
            "C:\\x.png",
            "//server/x",
            "emoji/../../x",
            "https://host/x",
            "/x",
        ] {
            assert!(relative(p).is_err(), "{p}");
        }
        assert_eq!(
            relative("emoji\\001.png").unwrap(),
            PathBuf::from("emoji/001.png")
        );
    }
    #[test]
    fn static_definition_keeps_identity_height_and_skips_icons() {
        let d = echo("loadRegistry('emoji', e=>e.meta.name,[{meta:{namespace:'emoji'},image:{is_emoji:true},content:['😃']},{meta:{namespace:'sp',title:{text:'像素'},author:{name:'Sheep'},license:{name:'CC'}},path:{images:'res/emoji/',translate:'sheep.'},image:{size:{height:{value:'1.35em'}}},content:[{type:'group'},{name:'happy',title:{translate:'happy'},path:'002.png'}]},{meta:{namespace:'material'},content:[{icon:'material:happy'}]}]);", false, None, None).unwrap();
        assert_eq!(d.entries.len(), 1);
        assert_eq!(d.entries[0].lib, "sp");
        assert_eq!(d.entries[0].height_em, 1.35);
        assert_eq!(d.entries[0].path, PathBuf::from("res/emoji/002.png"));
    }
    #[test]
    fn extension_rebases_images_and_translates() {
        let d = echo(
            r#"extensionManager.load({
            meta:{name:'pack'},
            register_hook:{loaded:[{registry:'echolive:emoji',value:{pack:{
                meta:{namespace:'sprel'},
                path:{images:'extensions/pack/emoji/',translate:'sheep.emotelab.'},
                image:{size:{height:{value:'4em'}}},
                content:[{name:'laugh',title:{translate:'laugh'},path:'001.gif'}]
            }}}]},
            localization_patch:{'zho-Hans':{emoji:{sheep:{emotelab:{emoji:{laugh:'笑'}}}}}}
        });"#,
            true,
            None,
            None,
        )
        .unwrap();
        assert_eq!(d.entries[0].path, PathBuf::from("emoji/001.gif"));
        assert_eq!(d.entries[0].translations["zh-CN"], "笑");
        assert_eq!(d.entries[0].height_em, 4.0);
    }
}
