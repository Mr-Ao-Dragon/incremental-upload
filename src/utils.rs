use std::collections::HashMap;

pub fn replace_variables(text: &str, vars: &HashMap<String, String>) -> String {
    let mut result = text.to_owned();
    let mut replaced;

    for _i in 0..1000 {
        replaced = false;

        for k in vars.keys() {
            let pattern = "$".to_string() + k;
            let new = result.replace(&pattern[..], &vars[k][..]);
            replaced |= result != new;
            result = new;
        }
        
        if !replaced {
            break;
        }
    }

    result
}

pub fn command_split(command: &str) -> Vec<String> {
    let mut split = Vec::<String>::new();
    let divided = command.split(" ").collect::<Vec<&str>>();

    let mut escaping = false;
    let mut buf = "".to_string();

    for d in divided {
        let s = d.starts_with("\"");
        let e = d.ends_with("\"");

        if s && !e {
            escaping = true;
            buf += &d[1..];
        } else if e && !s {
            escaping = false;
            buf += &(" ".to_string() + &d[..d.len() - 1]);
            split.push(buf.to_owned());
            buf.clear();
        } else {
            if escaping {
                buf += &(" ".to_string() + d);
            } else {
                let s_index = if s { 1 } else { 0 };
                let e_index = if e { d.len() - 1 } else { d.len() - 0 };
                split.push(d[s_index .. e_index].to_owned());
            }
        }
    }

    split
}

pub fn get_dirname(path: &str) -> Option<&str> {
    if let Some(pos) = path.rfind("/") {
        return Some(&path[..pos])
    }

    None
}

pub fn get_basename(path: &str) -> &str {
    if let Some(pos) = path.rfind("/") {
        return &path[pos + 1..]
    }

    path
}