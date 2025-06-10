use crate::state::location::FileKrakenLocation;
use std::path::Path;

pub fn is_path_parent(child: &str, parent: &str) -> bool {
    let child_path = Path::new(child);
    let parent_path = Path::new(parent);
    let mut child_path = child_path.parent();
    while let Some(path) = child_path {
        if path == parent_path {
            return true;
        }
        child_path = path.parent();
    }
    false
}

pub fn get_longest_parent_path<'a>(
    child: &str,
    parents: impl IntoIterator<Item = &'a FileKrakenLocation>,
) -> Option<String> {
    let mut file_parent_location = String::default();
    // get locations containing this file
    parents.into_iter().for_each(|location| {
        if is_path_parent(child, &location.path) && location.path.len() > file_parent_location.len()
        {
            file_parent_location = location.path.clone();
        }
    });
    if !file_parent_location.is_empty() {
        Some(file_parent_location)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::location::{
        FileKrakenLocation, FileKrakenLocationState, FileKrakenLocationType,
    };

    #[test]
    fn test_is_path_parent_basic() {
        assert!(is_path_parent("/foo/bar/baz.txt", "/foo"));
        assert!(is_path_parent("/foo/bar/baz.txt", "/foo/bar"));
        assert!(!is_path_parent("/foo/bar/baz.txt", "/fob"));
        assert!(!is_path_parent("/foo/bar", "/foo/bar"));
    }

    #[test]
    fn test_get_longest_parent_path() {
        let locations = vec![
            FileKrakenLocation {
                path: "/foo".to_string(),
                location_type: FileKrakenLocationType::Normal,
                location_state: FileKrakenLocationState::Scanned,
                parent_location_path: None,
            },
            FileKrakenLocation {
                path: "/foo/bar".to_string(),
                location_type: FileKrakenLocationType::Preferred,
                location_state: FileKrakenLocationState::Scanned,
                parent_location_path: Some("/foo".to_string()),
            },
        ];

        let res = get_longest_parent_path("/foo/bar/baz/file.txt", locations.iter()).unwrap();
        assert_eq!(res, "/foo/bar");
        let none_res = get_longest_parent_path("/bar/baz.txt", locations.iter());
        assert!(none_res.is_none());
    }
}
