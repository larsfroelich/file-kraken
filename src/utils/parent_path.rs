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
    if file_parent_location.len() > 0 {
        Some(file_parent_location)
    } else {
        None
    }
}
