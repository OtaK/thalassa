use thalassa::io::DepthAwareReader;

pub fn btd<'a>(bytes: &'a [u8]) -> DepthAwareReader<'a, &'a [u8]> {
    DepthAwareReader::from_reader(bytes)
}
