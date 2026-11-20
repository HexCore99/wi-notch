use ::windows::Storage::Streams::{DataReader, IRandomAccessStreamWithContentType};

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
