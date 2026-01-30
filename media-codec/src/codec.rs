use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::{Arc, LazyLock, RwLock},
};

use media_codec_types::{CodecBuilder, CodecID, CodecSpec};
use media_core::{invalid_error, not_found_error, Result};

pub(crate) struct CodecList<S: CodecSpec, B: ?Sized + CodecBuilder<S> = dyn CodecBuilder<S>> {
    pub(crate) codecs: HashMap<CodecID, Vec<Arc<B>>>,
    pub(crate) _marker: PhantomData<S>,
}

pub(crate) type LazyCodecList<S, B = dyn CodecBuilder<S>> = LazyLock<RwLock<CodecList<S, B>>>;

pub(crate) fn register_codec<S, B>(codec_list: &LazyCodecList<S, B>, builder: Arc<B>, default: bool) -> Result<()>
where
    S: CodecSpec,
    B: ?Sized + CodecBuilder<S>,
{
    let mut codec_list = codec_list.write().map_err(|err| invalid_error!(err.to_string()))?;
    for &id in builder.ids() {
        let list = codec_list.codecs.entry(id).or_default();

        if default {
            list.insert(0, builder.clone());
        } else {
            list.push(builder.clone());
        }
    }

    Ok(())
}

pub(crate) fn find_codec<S, B>(codec_list: &LazyCodecList<S, B>, id: CodecID) -> Result<Arc<B>>
where
    S: CodecSpec,
    B: ?Sized + CodecBuilder<S>,
{
    let codec_list = codec_list.read().map_err(|err| invalid_error!(err.to_string()))?;

    if let Some(builders) = codec_list.codecs.get(&id) {
        if let Some(builder) = builders.first() {
            return Ok(builder.clone());
        }
    }

    Err(not_found_error!(format!("codec: {:?}", id)))
}

pub(crate) fn find_codec_by_name<S, B>(codec_list: &LazyCodecList<S, B>, name: &str) -> Result<Arc<B>>
where
    S: CodecSpec,
    B: ?Sized + CodecBuilder<S>,
{
    let codec_list = codec_list.read().map_err(|err| invalid_error!(err.to_string()))?;

    for builders in codec_list.codecs.values() {
        for builder in builders {
            if builder.name() == name {
                return Ok(builder.clone());
            }
        }
    }

    Err(not_found_error!(format!("codec: {}", name)))
}
