use roguestill_core::noita::{parse_stats, Snapshot};
use std::{fs, io, path::Path, time::UNIX_EPOCH};

fn scan(path: &Path) -> io::Result<(usize, u64)> {
    let mut count = 0;
    let mut latest = fs::metadata(path)?
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() {
            continue;
        }
        if entry.file_type()?.is_dir() {
            let (nested, modified) = scan(&entry.path())?;
            count += nested;
            latest = latest.max(modified);
        } else {
            count += 1;
            latest = latest.max(
                entry
                    .metadata()?
                    .modified()?
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );
        }
    }
    Ok((count, latest))
}
pub fn read(path: &Path) -> io::Result<Snapshot> {
    let (_, last_write_time) = scan(path)?;
    let world = path.join("world");
    let world_file_count = if world.exists() { scan(&world)?.0 } else { 0 };
    let mut sessions = Vec::new();
    let directory = path.join("stats/sessions");
    if directory.exists() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(start) = name.strip_suffix("_stats.xml") {
                let xml = fs::read_to_string(entry.path())?;
                sessions.push(parse_stats(start, &xml).map_err(io::Error::other)?);
            }
        }
    }
    Ok(Snapshot {
        sessions,
        world_file_count,
        last_write_time,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_real_save_layout_and_ignores_kills_xml() {
        let path = std::env::temp_dir().join(format!("roguestill-snapshot-{}", std::process::id()));
        fs::create_dir_all(path.join("world/nested")).unwrap();
        fs::create_dir_all(path.join("stats/sessions")).unwrap();
        fs::write(path.join("world/nested/chunk"), "world").unwrap();
        fs::write(
            path.join("stats/sessions/run_stats.xml"),
            "<Stats playtime='30' dead='1'/>",
        )
        .unwrap();
        fs::write(path.join("stats/sessions/run_kills.xml"), "ignored").unwrap();
        let snapshot = read(&path).unwrap();
        assert_eq!(snapshot.world_file_count, 1);
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].run_start_name, "run");
        fs::remove_dir_all(path).unwrap();
    }
}
