use crate::core::requests::RequestContext;
//TODO:: need implementation

#[derive(Debug, Clone, Serialize)]
struct GetUserProfilePhotos<'a> {
    #[serde(skip_serializing)]
    ctx: RequestContext<'a>,
}
