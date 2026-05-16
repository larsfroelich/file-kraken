use crate::state::location::FileKrakenLocation;
use std::path::Path;

pub fn is_ancestor_of(ancestor: &str, descendant: &str) -> bool {
    let descendant_path = Path::new(descendant);
    let ancestor_path = Path::new(ancestor);
    let mut descendant_path = descendant_path.parent();
    while let Some(path) = descendant_path {
        if path == ancestor_path {
            return true;
        }
        descendant_path = path.parent();
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
        if is_ancestor_of(&location.path, child) && location.path.len() > file_parent_location.len()
        {
            file_parent_location = location.path.clone();
        }
    });
    if file_parent_location.is_empty() {
        None
    } else {
        Some(file_parent_location)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::location::{
        FileKrakenLocation, FileKrakenLocationState, FileKrakenLocationType,
    };

    #[test]
    fn test_is_ancestor_of_basic() {
        assert!(is_ancestor_of("/foo", "/foo/bar/baz.txt"));
        assert!(is_ancestor_of("/foo/bar", "/foo/bar/baz.txt"));
        assert!(!is_ancestor_of("/fob", "/foo/bar/baz.txt"));
        assert!(!is_ancestor_of("/foo/bar", "/foo/bar"));
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
