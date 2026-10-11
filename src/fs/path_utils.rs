use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbItem {
    pub label: String,
    pub path: String,
    pub is_last: bool,
}

/// Normalizes a path string, respecting Windows drives (`C:\`) and UNC paths (`\\server\share`).
pub fn normalize_path(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return ".".to_string();
    }

    // Check for Windows drive prefix like "C:" or "C:\"
    let is_windows_drive = raw.len() >= 2
        && raw.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
        && raw.chars().nth(1) == Some(':');

    // Check for UNC path like "\\server\share" or "//server/share"
    let is_unc = (raw.starts_with(r"\\") || raw.starts_with("//")) && raw.len() > 2;

    if is_unc {
        let normalized = raw.replace('/', r"\");
        return normalized;
    }

    if is_windows_drive {
        let drive = &raw[..2].to_uppercase();
        let rest = &raw[2..];
        let mut rest_norm = rest.replace('/', r"\");
        while rest_norm.contains(r"\\") {
            rest_norm = rest_norm.replace(r"\\", r"\");
        }
        if rest_norm.is_empty() || !rest_norm.starts_with(r"\") {
            format!(r"{}\{}", drive, rest_norm.trim_start_matches(r"\"))
        } else {
            format!(r"{}{}", drive, rest_norm)
        }
    } else {
        // POSIX / Remote path
        let mut cleaned = raw.replace(r"\", "/");
        if !cleaned.starts_with('/') && !cleaned.starts_with('.') {
            // Keep as is or add leading slash if absolute
        }
        while cleaned.contains("//") {
            cleaned = cleaned.replace("//", "/");
        }
        cleaned
    }
}

/// Returns the parent path of a path string.
pub fn get_parent_path(raw: &str, is_remote: bool) -> Option<String> {
    if is_remote {
        let path = raw.replace(r"\", "/");
        let path = path.trim_end_matches('/');
        if path.is_empty() || path == "/" {
            return None;
        }
        match path.rfind('/') {
            Some(0) => Some("/".to_string()),
            Some(pos) => Some(path[..pos].to_string()),
            None => Some("/".to_string()),
        }
    } else {
        let p = Path::new(raw);
        if let Some(parent) = p.parent() {
            let s = parent.to_string_lossy().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        } else {
            None
        }
    }
}

/// Joins a parent directory path with a child name.
pub fn join_path(parent: &str, child: &str, is_remote: bool) -> String {
    if is_remote {
        let mut p = parent.replace(r"\", "/");
        if !p.ends_with('/') {
            p.push('/');
        }
        p.push_str(child.trim_start_matches('/'));
        p
    } else {
        let p = Path::new(parent);
        p.join(child).to_string_lossy().to_string()
    }
}

/// Breaks a path into breadcrumb items for navigation UI.
pub fn build_breadcrumbs(path_str: &str, is_remote: bool) -> Vec<BreadcrumbItem> {
    let mut crumbs = Vec::new();
    if path_str.is_empty() {
        return crumbs;
    }

    if is_remote {
        let normalized = path_str.replace(r"\", "/");
        let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();
        crumbs.push(BreadcrumbItem {
            label: "/".to_string(),
            path: "/".to_string(),
            is_last: parts.is_empty(),
        });
        let mut current = String::new();
        for (i, part) in parts.iter().enumerate() {
            current.push('/');
            current.push_str(part);
            let is_last = i == parts.len() - 1;
            crumbs.push(BreadcrumbItem {
                label: part.to_string(),
                path: current.clone(),
                is_last,
            });
        }
    } else {
        let p = Path::new(path_str);
        let mut current = PathBuf::new();
        let components: Vec<Component> = p.components().collect();

        for (i, comp) in components.iter().enumerate() {
            current.push(comp.as_os_str());
            let label = match comp {
                Component::Prefix(prefix) => prefix.as_os_str().to_string_lossy().to_string(),
                Component::RootDir => "/".to_string(),
                Component::CurDir => ".".to_string(),
                Component::ParentDir => "..".to_string(),
                Component::Normal(name) => name.to_string_lossy().to_string(),
            };
            let is_last = i == components.len() - 1;
            crumbs.push(BreadcrumbItem {
                label,
                path: current.to_string_lossy().to_string(),
                is_last,
            });
        }
    }
    crumbs
}

/// Detects available drives on Windows (e.g. `["C:\\", "D:\\"]`) or roots on Unix (`["/"]`).
pub fn list_system_roots() -> Vec<String> {
    #[cfg(windows)]
    {
        let mut roots = Vec::new();
        for c in b'A'..=b'Z' {
            let drive = format!("{}:\\", c as char);
            if Path::new(&drive).exists() {
                roots.push(drive);
            }
        }
        if roots.is_empty() {
            roots.push("C:\\".to_string());
        }
        roots
    }
    #[cfg(not(windows))]
    {
        // On Unix, provide standard root and home
        vec!["/".to_string()]
    }
}

/// Checks if a filename is valid according to Windows filename rules.
pub fn is_valid_filename(name: &str) -> bool {
    if name.is_empty() || name == "." || name == ".." {
        return false;
    }
    // Windows prohibited characters: < > : " / \ | ? *
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    if name.chars().any(|c| invalid_chars.contains(&c)) {
        return false;
    }
    // Windows reserved names: CON, PRN, AUX, NUL, COM1-9, LPT1-9
    let base = name.split('.').next().unwrap_or("").to_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.contains(&base.as_str()) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_filename() {
        assert!(is_valid_filename("hello.txt"));
        assert!(is_valid_filename("test_file_123.rs"));
        assert!(!is_valid_filename(""));
        assert!(!is_valid_filename("."));
        assert!(!is_valid_filename(".."));
        assert!(!is_valid_filename("foo/bar"));
        assert!(!is_valid_filename("foo\\bar"));
        assert!(!is_valid_filename("foo:bar"));
        assert!(!is_valid_filename("CON.txt"));
        assert!(!is_valid_filename("aux"));
    }

    #[test]
    fn test_remote_breadcrumbs() {
        let crumbs = build_breadcrumbs("/var/log/nginx", true);
        assert_eq!(crumbs.len(), 4);
        assert_eq!(crumbs[0].path, "/");
        assert_eq!(crumbs[1].path, "/var");
        assert_eq!(crumbs[2].path, "/var/log");
        assert_eq!(crumbs[3].path, "/var/log/nginx");
        assert!(crumbs[3].is_last);
    }

    #[test]
    fn test_parent_path_remote() {
        assert_eq!(get_parent_path("/var/log", true), Some("/var".to_string()));
        assert_eq!(get_parent_path("/var", true), Some("/".to_string()));
        assert_eq!(get_parent_path("/", true), None);
    }
}
