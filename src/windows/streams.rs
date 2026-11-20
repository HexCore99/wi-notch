use std::{
    ffi::OsString,
    fs::File,
    io::Read,
    os::windows::ffi::OsStringExt,
    path::{Component, PathBuf, Prefix},
};

use ::windows::{
    Storage::Streams::{DataReader, IRandomAccessStreamWithContentType},
    Win32::UI::Shell::PathCreateFromUrlW,
    core::{HSTRING, PWSTR},
};

pub(super) async fn read_image_stream(
    stream: IRandomAccessStreamWithContentType,
    max_bytes: u32,
) -> Option<(String, Vec<u8>)> {
    let size = u32::try_from(stream.Size().ok()?).ok()?;
    if size == 0 || size > max_bytes {
        return None;
    }

    let input = stream.GetInputStreamAt(0).ok()?;
    let reader = DataReader::CreateDataReader(&input).ok()?;
    let loaded = reader.LoadAsync(size).ok()?.await.ok()?;
    if loaded != size {
        return None;
    }

    let mut bytes = vec![0; loaded as usize];
    reader.ReadBytes(&mut bytes).ok()?;
    let content_type = stream.ContentType().unwrap_or_default().to_string();
    Some((content_type, bytes))
}

pub(super) fn read_local_image(source: &str, max_bytes: u64) -> Option<Vec<u8>> {
    let path = local_image_path(source)?;
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= max_bytes).then_some(bytes)
}

fn local_image_path(source: &str) -> Option<PathBuf> {
    let path = if source.starts_with("file:///") {
        let input = HSTRING::from(source);
        let mut buffer = [0; 4096];
        let mut len = buffer.len() as u32;
        unsafe { PathCreateFromUrlW(&input, PWSTR(buffer.as_mut_ptr()), &mut len, 0) }.ok()?;
        PathBuf::from(OsString::from_wide(buffer.get(..len as usize)?))
    } else {
        PathBuf::from(source)
    };
    // Toasts can also specify web URLs and UNC paths. Only load local drive paths.
    let drive_path = path.is_absolute()
        && matches!(path.components().next(), Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_)));
    drive_path.then_some(path)
}

#[cfg(test)]
mod tests {
    use super::local_image_path;
    use std::path::PathBuf;

    #[test]
    fn images_accept_local_paths_and_decode_file_uris() {
        let path = PathBuf::from(r"C:\Temp\toast picture.tmp");
        assert_eq!(
            local_image_path(r"C:\Temp\toast picture.tmp"),
            Some(path.clone())
        );
        assert_eq!(
            local_image_path("file:///C:/Temp/toast%20picture.tmp"),
            Some(path)
        );
        for source in [
            "https://example.com/picture.png",
            "file://server/share/picture.png",
            r"\\server\share\picture.png",
            r"C:picture.png",
        ] {
            assert_eq!(local_image_path(source), None);
        }
    }
}
