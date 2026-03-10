// Reads and displays objects from the store by SHA
use crate::object::Object;
use crate::repo;

pub fn execute(sha: &str, pretty: bool, show_type: bool, show_size: bool) -> Result<(), String> {
    let flags_set = [pretty, show_type, show_size].iter().filter(|&&f| f).count();
    if flags_set == 0 {
        return Err("one of -p, -t, or -s is required".into());
    }
    if flags_set > 1 {
        return Err("only one of -p, -t, -s can be used at a time".into());
    }

    let vrit_dir = repo::find_vrit_dir()?;
    let obj = Object::read_from_store(&vrit_dir, sha)?;

    if show_type {
        println!("{}", obj.type_str());
    } else if show_size {
        println!("{}", obj.serialize_body().len());
    } else {
        print!("{obj}");
    }

    Ok(())
}
