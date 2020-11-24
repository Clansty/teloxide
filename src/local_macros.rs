#[macro_use]
macro_rules! forward_to_unsuported_ty {
    (
        supported: $supported:expr;
        simple { $( $method:ident $arg:ty )* }
        unit { $( $method1:ident $ty:expr )* }
        compound {
            $( $method2:ident $( <$T:ident: ?Sized + Serialize> )? ( $( $args:tt )* ) -> $ret:ty => $message:expr )*
        }
    ) => {
        $(
            fn $method(self, _: $arg) -> Result<Self::Ok, Self::Error> {
                Err(Self::Error::UnsupportedType {
                    ty: stringify!($arg),
                    supported: $supported,
                })
            }
        )+

        $(
            fn $method1(self) -> Result<Self::Ok, Self::Error> {
                Err(Self::Error::UnsupportedType {
                    ty: $ty,
                    supported: $supported,
                })
            }
        )+

        $(
            fn $method2 $( <$T: ?Sized + Serialize> )? (self, $( $args )*) -> Result<$ret, Self::Error> {
                Err(Self::Error::UnsupportedType {
                    ty: $message,
                    supported: $supported,
                })
            }
        )+
    };
}

#[macro_use]
macro_rules! req_future {
    (
        $v2:vis def: | $( $arg:ident: $ArgTy:ty ),* $(,)? | $body:block

        $(#[$($meta:tt)*])*
        $v:vis $i:ident<$T:ident> ($inner:ident) -> $Out:ty
        $(where $($wh:tt)*)?
    ) => {
        #[pin_project::pin_project]
        $v struct $i<$T>
        $(where $($wh)*)?
        {
            #[pin]
            inner: $inner::$i<$T>
        }

        impl<$T> $i<$T>
        $(where $($wh)*)?
        {
            $v2 fn new($( $arg: $ArgTy ),*) -> Self {
                Self { inner: $inner::def($( $arg ),*) }
            }
        }

        // HACK(waffle): workaround for https://github.com/rust-lang/rust/issues/55997
        mod $inner {
            #![allow(type_alias_bounds)]

            // Mostly to bring `use`s
            #[allow(unused_imports)]
            use super::{*, $i as _};

            #[cfg(feature = "nightly")]
            pub(crate) type $i<$T>
            $(where $($wh)*)? = impl ::core::future::Future<Output = $Out>;

            #[cfg(feature = "nightly")]
            pub(crate) fn def<$T>($( $arg: $ArgTy ),*) -> $i<$T>
            $(where $($wh)*)?
            {
                $body
            }

            #[cfg(not(feature = "nightly"))]
            pub(crate) type $i<$T>
            $(where $($wh)*)?  = ::core::pin::Pin<Box<dyn ::core::future::Future<Output = $Out> + ::core::marker::Send + 'static>>;

            #[cfg(not(feature = "nightly"))]
            pub(crate) fn def<$T>($( $arg: $ArgTy ),*) -> $i<$T>
            $(where $($wh)*)?
            {
                Box::pin($body)
            }
        }

        impl<$T> ::core::future::Future for $i<$T>
        $(where $($wh)*)?
        {
            type Output = $Out;

            fn poll(self: ::core::pin::Pin<&mut Self>, cx: &mut ::core::task::Context<'_>) -> ::core::task::Poll<Self::Output> {
                let this = self.project();
                this.inner.poll(cx)
            }
        }

    };
}

/// Declares an item with a doc attribute computed by some macro expression.
/// This allows documentation to be dynamically generated based on input.
/// Necessary to work around https://github.com/rust-lang/rust/issues/52607.
#[macro_use]
macro_rules! calculated_doc {
    (
        $(
            #[doc = $doc:expr]
            $thing:item
        )*
    ) => (
        $(
            #[doc = $doc]
            $thing
        )*
    );
}

/// Declare payload type, implement `Payload` trait amd ::new method for it,
/// declare setters trait and implement it for all type which have payload.
#[macro_use]
macro_rules! impl_payload {
    (
        $(
            #[ $($method_meta:tt)* ]
        )*
        $vi:vis $Method:ident ($Setters:ident) => $Ret:ty {
            $(
                required {
                    $(
                        $(
                            #[ $($field_meta:tt)* ]
                        )*
                        $v:vis $fields:ident : $FTy:ty $([$conv:ident])?
                        ,
                    )*
                }
            )?

            $(
                optional {
                    $(
                        $(
                            #[ $($opt_field_meta:tt)* ]
                        )*
                        $opt_v:vis $opt_fields:ident : $OptFTy:ty $([$opt_conv:ident])?
                    ),*
                    $(,)?
                }
            )?
        }
    ) => {
        #[serde_with_macros::skip_serializing_none]
        #[must_use = "Requests do nothing unless sent"]
        $(
            #[ $($method_meta)* ]
        )*
        $vi struct $Method {
            $(
                $(
                    $(
                        #[ $($field_meta)* ]
                    )*
                    $v $fields : $FTy,
                )*
            )?
            $(
                $(
                    $(
                        #[ $($opt_field_meta)* ]
                    )*
                    $opt_v $opt_fields : core::option::Option<$OptFTy>,
                )*
            )?
        }

        impl $Method {
            // We mirror Telegram API and can't do anything with too many arguments.
            #[allow(clippy::too_many_arguments)]
            // It's just easier for macros to generate such code.
            #[allow(clippy::redundant_field_names)]
            $vi fn new($($($fields : impl_payload!(@convert? $FTy $([$conv])?)),*)?) -> Self {
                Self {
                    $(
                        $(
                            $fields: impl_payload!(@convert_map ($fields) $([$conv])?),
                        )*
                    )?
                    $(
                        $(
                            $opt_fields: None,
                        )*
                    )?
                }
            }
        }

        impl $crate::requests::Payload for $Method {
            type Output = $Ret;

            const NAME: &'static str = stringify!($Method);
        }

        calculated_doc! {
            #[doc = concat!(
                "Setters for fields of [`",
                stringify!($Method),
                "`]"
            )]
            $vi trait $Setters: $crate::requests::HasPayload<Payload = $Method> + ::core::marker::Sized {
                $(
                    $(
                        impl_payload! { @setter $Method $fields : $FTy $([$conv])? }
                    )*
                )?
                $(
                    $(
                        impl_payload! { @setter_opt $Method $opt_fields : $OptFTy $([$opt_conv])? }
                    )*
                )?
            }
        }

        impl<P> $Setters for P where P: crate::requests::HasPayload<Payload = $Method> {}
    };
    (@setter_opt $Method:ident $field:ident : $FTy:ty [into]) => {
        calculated_doc! {
            #[doc = concat!(
                "Setter for [`",
                stringify!($field),
                "`](",
                stringify!($Method),
                "::",
                stringify!($field),
                ") field."
            )]
            fn $field<T>(mut self, value: T) -> Self
            where
                T: Into<$FTy>,
            {
                self.payload_mut().$field = Some(value.into());
                self
            }
        }
    };
    (@setter_opt $Method:ident $field:ident : $FTy:ty [collect]) => {
        calculated_doc! {
            #[doc = concat!(
                "Setter for [`",
                stringify!($field),
                "`](",
                stringify!($Method),
                "::",
                stringify!($field),
                ") field."
            )]
            fn $field<T>(mut self, value: T) -> Self
            where
                T: ::core::iter::IntoIterator<Item = <$FTy as ::core::iter::IntoIterator>::Item>,
            {
                self.payload_mut().$field = Some(value.into_iter().collect());
                self
            }
        }
    };
    (@setter_opt $Method:ident $field:ident : $FTy:ty) => {
        calculated_doc! {
            #[doc = concat!(
                "Setter for [`",
                stringify!($field),
                "`](",
                stringify!($Method),
                "::",
                stringify!($field),
                ") field."
            )]
            fn $field(mut self, value: $FTy) -> Self {
                self.payload_mut().$field = Some(value);
                self
            }
        }
    };
    (@setter $Method:ident $field:ident : $FTy:ty [into]) => {
        calculated_doc! {
            #[doc = concat!(
                "Setter for [`",
                stringify!($field),
                "`](",
                stringify!($Method),
                "::",
                stringify!($field),
                ") field."
            )]
            fn $field<T>(mut self, value: T) -> Self
            where
                T: Into<$FTy>,
            {
                self.payload_mut().$field = value.into();
                self
            }
        }
    };
    (@setter $Method:ident $field:ident : $FTy:ty [collect]) => {
        calculated_doc! {
            #[doc = concat!(
                "Setter for [`",
                stringify!($field),
                "`](",
                stringify!($Method),
                "::",
                stringify!($field),
                ") field."
            )]
            fn $field<T>(mut self, value: T) -> Self
            where
                T: ::core::iter::IntoIterator<Item = <$FTy as ::core::iter::IntoIterator>::Item>,
            {
                self.payload_mut().$field = value.into_iter().collect();
                self
            }
        }
    };
    (@setter $Method:ident $field:ident : $FTy:ty) => {
        calculated_doc! {
            #[doc = concat!(
                "Setter for [`",
                stringify!($field),
                "`](",
                stringify!($Method),
                "::",
                stringify!($field),
                ") field."
            )]
            fn $field(mut self, value: $FTy) -> Self {
                self.payload_mut().$field = value;
                self
            }
        }
    };
    (@convert? $T:ty [into]) => {
        impl ::core::convert::Into<$T>
    };
    (@convert? $T:ty [collect]) => {
        impl ::core::iter::IntoIterator<Item = <$T as ::core::iter::IntoIterator>::Item>
    };
    (@convert? $T:ty) => {
        $T
    };
    (@convert_map ($e:expr) [into]) => {
        $e.into()
    };
    (@convert_map ($e:expr) [collect]) => {
        $e.into_iter().collect()
    };
    (@convert_map ($e:expr)) => {
        $e
    };
}

#[macro_use]
// This macro is auto generated by `cg` <https://github.com/teloxide/cg> (fea4d31).
// **DO NOT EDIT THIS MACRO**,
// edit `cg` instead.
macro_rules! requester_forward {
    ($i:ident $(, $rest:ident )* $(,)? => $body:ident, $ty:ident ) => {
        requester_forward!(@method $i $body $ty);
        $(
            requester_forward!(@method $rest $body $ty);
        )*
    };
    (@method get_updates $body:ident $ty:ident) => {
        type GetUpdates = $ty![GetUpdates];

        fn get_updates<>(&self, ) -> Self::GetUpdates where  {
            let this = self;
            $body!(get_updates this ())
        }
    };
    (@method set_webhook $body:ident $ty:ident) => {
        type SetWebhook = $ty![SetWebhook];

        fn set_webhook<U, A>(&self, url: U, allowed_updates: A) -> Self::SetWebhook where U: Into<String>,
        A: IntoIterator<Item = AllowedUpdate> {
            let this = self;
            $body!(set_webhook this (url: U, allowed_updates: A))
        }
    };
    (@method delete_webhook $body:ident $ty:ident) => {
        type DeleteWebhook = $ty![DeleteWebhook];

        fn delete_webhook<>(&self, ) -> Self::DeleteWebhook where  {
            let this = self;
            $body!(delete_webhook this ())
        }
    };
    (@method get_webhook_info $body:ident $ty:ident) => {
        type GetWebhookInfo = $ty![GetWebhookInfo];

        fn get_webhook_info<>(&self, ) -> Self::GetWebhookInfo where  {
            let this = self;
            $body!(get_webhook_info this ())
        }
    };
    (@method get_me $body:ident $ty:ident) => {
        type GetMe = $ty![GetMe];

        fn get_me<>(&self, ) -> Self::GetMe where  {
            let this = self;
            $body!(get_me this ())
        }
    };
    (@method send_message $body:ident $ty:ident) => {
        type SendMessage = $ty![SendMessage];

        fn send_message<C, T>(&self, chat_id: C, text: T) -> Self::SendMessage where C: Into<ChatId>,
        T: Into<String> {
            let this = self;
            $body!(send_message this (chat_id: C, text: T))
        }
    };
    (@method forward_message $body:ident $ty:ident) => {
        type ForwardMessage = $ty![ForwardMessage];

        fn forward_message<C, F>(&self, chat_id: C, from_chat_id: F, message_id: i32) -> Self::ForwardMessage where C: Into<ChatId>,
        F: Into<ChatId> {
            let this = self;
            $body!(forward_message this (chat_id: C, from_chat_id: F, message_id: i32))
        }
    };
    (@method send_photo $body:ident $ty:ident) => {
        type SendPhoto = $ty![SendPhoto];

        fn send_photo<Ch, Ca>(&self, chat_id: Ch, photo: InputFile, caption: Ca) -> Self::SendPhoto where Ch: Into<ChatId>,
        Ca: Into<String> {
            let this = self;
            $body!(send_photo this (chat_id: Ch, photo: InputFile, caption: Ca))
        }
    };
    (@method send_audio $body:ident $ty:ident) => {
        type SendAudio = $ty![SendAudio];

        fn send_audio<Ch, Ca>(&self, chat_id: Ch, audio: InputFile, caption: Ca) -> Self::SendAudio where Ch: Into<ChatId>,
        Ca: Into<String> {
            let this = self;
            $body!(send_audio this (chat_id: Ch, audio: InputFile, caption: Ca))
        }
    };
    (@method send_document $body:ident $ty:ident) => {
        type SendDocument = $ty![SendDocument];

        fn send_document<Ch, Ca>(&self, chat_id: Ch, document: InputFile, caption: Ca) -> Self::SendDocument where Ch: Into<ChatId>,
        Ca: Into<String> {
            let this = self;
            $body!(send_document this (chat_id: Ch, document: InputFile, caption: Ca))
        }
    };
    (@method send_video $body:ident $ty:ident) => {
        type SendVideo = $ty![SendVideo];

        fn send_video<Ch, Ca>(&self, chat_id: Ch, video: InputFile, caption: Ca) -> Self::SendVideo where Ch: Into<ChatId>,
        Ca: Into<String> {
            let this = self;
            $body!(send_video this (chat_id: Ch, video: InputFile, caption: Ca))
        }
    };
    (@method send_animation $body:ident $ty:ident) => {
        type SendAnimation = $ty![SendAnimation];

        fn send_animation<Ch, Ca>(&self, chat_id: Ch, animation: InputFile, caption: Ca) -> Self::SendAnimation where Ch: Into<ChatId>,
        Ca: Into<String> {
            let this = self;
            $body!(send_animation this (chat_id: Ch, animation: InputFile, caption: Ca))
        }
    };
    (@method send_voice $body:ident $ty:ident) => {
        type SendVoice = $ty![SendVoice];

        fn send_voice<Ch, Ca>(&self, chat_id: Ch, voice: InputFile, caption: Ca) -> Self::SendVoice where Ch: Into<ChatId>,
        Ca: Into<String> {
            let this = self;
            $body!(send_voice this (chat_id: Ch, voice: InputFile, caption: Ca))
        }
    };
    (@method send_video_note $body:ident $ty:ident) => {
        type SendVideoNote = $ty![SendVideoNote];

        fn send_video_note<C>(&self, chat_id: C, video_note: InputFile) -> Self::SendVideoNote where C: Into<ChatId> {
            let this = self;
            $body!(send_video_note this (chat_id: C, video_note: InputFile))
        }
    };
    (@method send_media_group $body:ident $ty:ident) => {
        type SendMediaGroup = $ty![SendMediaGroup];

        fn send_media_group<C, M>(&self, chat_id: C, media: M) -> Self::SendMediaGroup where C: Into<ChatId>,
        M: IntoIterator<Item = InputMedia> {
            let this = self;
            $body!(send_media_group this (chat_id: C, media: M))
        }
    };
    (@method send_location $body:ident $ty:ident) => {
        type SendLocation = $ty![SendLocation];

        fn send_location<C>(&self, chat_id: C, latitude: f64, longitude: f64, live_period: u32) -> Self::SendLocation where C: Into<ChatId> {
            let this = self;
            $body!(send_location this (chat_id: C, latitude: f64, longitude: f64, live_period: u32))
        }
    };
    (@method edit_message_live_location $body:ident $ty:ident) => {
        type EditMessageLiveLocation = $ty![EditMessageLiveLocation];

        fn edit_message_live_location<C>(&self, chat_id: C, message_id: i32, latitude: f64, longitude: f64) -> Self::EditMessageLiveLocation where C: Into<ChatId> {
            let this = self;
            $body!(edit_message_live_location this (chat_id: C, message_id: i32, latitude: f64, longitude: f64))
        }
    };
    (@method edit_message_live_location_inline $body:ident $ty:ident) => {
        type EditMessageLiveLocationInline = $ty![EditMessageLiveLocationInline];

        fn edit_message_live_location_inline<I>(&self, inline_message_id: I, latitude: f64, longitude: f64) -> Self::EditMessageLiveLocationInline where I: Into<String> {
            let this = self;
            $body!(edit_message_live_location_inline this (inline_message_id: I, latitude: f64, longitude: f64))
        }
    };
    (@method stop_message_live_location $body:ident $ty:ident) => {
        type StopMessageLiveLocation = $ty![StopMessageLiveLocation];

        fn stop_message_live_location<C>(&self, chat_id: C, message_id: i32, latitude: f64, longitude: f64) -> Self::StopMessageLiveLocation where C: Into<ChatId> {
            let this = self;
            $body!(stop_message_live_location this (chat_id: C, message_id: i32, latitude: f64, longitude: f64))
        }
    };
    (@method stop_message_live_location_inline $body:ident $ty:ident) => {
        type StopMessageLiveLocationInline = $ty![StopMessageLiveLocationInline];

        fn stop_message_live_location_inline<I>(&self, inline_message_id: I, latitude: f64, longitude: f64) -> Self::StopMessageLiveLocationInline where I: Into<String> {
            let this = self;
            $body!(stop_message_live_location_inline this (inline_message_id: I, latitude: f64, longitude: f64))
        }
    };
    (@method send_venue $body:ident $ty:ident) => {
        type SendVenue = $ty![SendVenue];

        fn send_venue<C, T, A>(&self, chat_id: C, latitude: f64, longitude: f64, title: T, address: A) -> Self::SendVenue where C: Into<ChatId>,
        T: Into<String>,
        A: Into<String> {
            let this = self;
            $body!(send_venue this (chat_id: C, latitude: f64, longitude: f64, title: T, address: A))
        }
    };
    (@method send_contact $body:ident $ty:ident) => {
        type SendContact = $ty![SendContact];

        fn send_contact<C>(&self, chat_id: C, phone_number: f64, first_name: f64) -> Self::SendContact where C: Into<ChatId> {
            let this = self;
            $body!(send_contact this (chat_id: C, phone_number: f64, first_name: f64))
        }
    };
    (@method send_poll $body:ident $ty:ident) => {
        type SendPoll = $ty![SendPoll];

        fn send_poll<C, Q, O>(&self, chat_id: C, question: Q, options: O, type_: PollType) -> Self::SendPoll where C: Into<ChatId>,
        Q: Into<String>,
        O: IntoIterator<Item = String> {
            let this = self;
            $body!(send_poll this (chat_id: C, question: Q, options: O, type_: PollType))
        }
    };
    (@method send_dice $body:ident $ty:ident) => {
        type SendDice = $ty![SendDice];

        fn send_dice<C>(&self, chat_id: C, emoji: DiceEmoji) -> Self::SendDice where C: Into<ChatId> {
            let this = self;
            $body!(send_dice this (chat_id: C, emoji: DiceEmoji))
        }
    };
    (@method send_chat_action $body:ident $ty:ident) => {
        type SendChatAction = $ty![SendChatAction];

        fn send_chat_action<C>(&self, chat_id: C, action: ChatAction) -> Self::SendChatAction where C: Into<ChatId> {
            let this = self;
            $body!(send_chat_action this (chat_id: C, action: ChatAction))
        }
    };
    (@method get_user_profile_photos $body:ident $ty:ident) => {
        type GetUserProfilePhotos = $ty![GetUserProfilePhotos];

        fn get_user_profile_photos<>(&self, user_id: i32) -> Self::GetUserProfilePhotos where  {
            let this = self;
            $body!(get_user_profile_photos this (user_id: i32))
        }
    };
    (@method get_file $body:ident $ty:ident) => {
        type GetFile = $ty![GetFile];

        fn get_file<F>(&self, file_id: F) -> Self::GetFile where F: Into<String> {
            let this = self;
            $body!(get_file this (file_id: F))
        }
    };
    (@method kick_chat_member $body:ident $ty:ident) => {
        type KickChatMember = $ty![KickChatMember];

        fn kick_chat_member<C>(&self, chat_id: C, user_id: i32) -> Self::KickChatMember where C: Into<ChatId> {
            let this = self;
            $body!(kick_chat_member this (chat_id: C, user_id: i32))
        }
    };
    (@method unban_chat_member $body:ident $ty:ident) => {
        type UnbanChatMember = $ty![UnbanChatMember];

        fn unban_chat_member<C>(&self, chat_id: C, user_id: i32) -> Self::UnbanChatMember where C: Into<ChatId> {
            let this = self;
            $body!(unban_chat_member this (chat_id: C, user_id: i32))
        }
    };
    (@method restrict_chat_member $body:ident $ty:ident) => {
        type RestrictChatMember = $ty![RestrictChatMember];

        fn restrict_chat_member<C>(&self, chat_id: C, user_id: i32, permissions: ChatPermissions) -> Self::RestrictChatMember where C: Into<ChatId> {
            let this = self;
            $body!(restrict_chat_member this (chat_id: C, user_id: i32, permissions: ChatPermissions))
        }
    };
    (@method promote_chat_member $body:ident $ty:ident) => {
        type PromoteChatMember = $ty![PromoteChatMember];

        fn promote_chat_member<C>(&self, chat_id: C, user_id: i32) -> Self::PromoteChatMember where C: Into<ChatId> {
            let this = self;
            $body!(promote_chat_member this (chat_id: C, user_id: i32))
        }
    };
    (@method set_chat_administrator_custom_title $body:ident $ty:ident) => {
        type SetChatAdministratorCustomTitle = $ty![SetChatAdministratorCustomTitle];

        fn set_chat_administrator_custom_title<Ch, Cu>(&self, chat_id: Ch, user_id: i32, custom_title: Cu) -> Self::SetChatAdministratorCustomTitle where Ch: Into<ChatId>,
        Cu: Into<String> {
            let this = self;
            $body!(set_chat_administrator_custom_title this (chat_id: Ch, user_id: i32, custom_title: Cu))
        }
    };
    (@method set_chat_permissions $body:ident $ty:ident) => {
        type SetChatPermissions = $ty![SetChatPermissions];

        fn set_chat_permissions<C>(&self, chat_id: C, permissions: ChatPermissions) -> Self::SetChatPermissions where C: Into<ChatId> {
            let this = self;
            $body!(set_chat_permissions this (chat_id: C, permissions: ChatPermissions))
        }
    };
    (@method export_chat_invite_link $body:ident $ty:ident) => {
        type ExportChatInviteLink = $ty![ExportChatInviteLink];

        fn export_chat_invite_link<C>(&self, chat_id: C) -> Self::ExportChatInviteLink where C: Into<ChatId> {
            let this = self;
            $body!(export_chat_invite_link this (chat_id: C))
        }
    };
    (@method set_chat_photo $body:ident $ty:ident) => {
        type SetChatPhoto = $ty![SetChatPhoto];

        fn set_chat_photo<C>(&self, chat_id: C, photo: InputFile) -> Self::SetChatPhoto where C: Into<ChatId> {
            let this = self;
            $body!(set_chat_photo this (chat_id: C, photo: InputFile))
        }
    };
    (@method delete_chat_photo $body:ident $ty:ident) => {
        type DeleteChatPhoto = $ty![DeleteChatPhoto];

        fn delete_chat_photo<C>(&self, chat_id: C) -> Self::DeleteChatPhoto where C: Into<ChatId> {
            let this = self;
            $body!(delete_chat_photo this (chat_id: C))
        }
    };
    (@method set_chat_title $body:ident $ty:ident) => {
        type SetChatTitle = $ty![SetChatTitle];

        fn set_chat_title<C, T>(&self, chat_id: C, title: T) -> Self::SetChatTitle where C: Into<ChatId>,
        T: Into<String> {
            let this = self;
            $body!(set_chat_title this (chat_id: C, title: T))
        }
    };
    (@method set_chat_description $body:ident $ty:ident) => {
        type SetChatDescription = $ty![SetChatDescription];

        fn set_chat_description<C>(&self, chat_id: C) -> Self::SetChatDescription where C: Into<ChatId> {
            let this = self;
            $body!(set_chat_description this (chat_id: C))
        }
    };
    (@method pin_chat_message $body:ident $ty:ident) => {
        type PinChatMessage = $ty![PinChatMessage];

        fn pin_chat_message<C>(&self, chat_id: C, message_id: i32) -> Self::PinChatMessage where C: Into<ChatId> {
            let this = self;
            $body!(pin_chat_message this (chat_id: C, message_id: i32))
        }
    };
    (@method unpin_chat_message $body:ident $ty:ident) => {
        type UnpinChatMessage = $ty![UnpinChatMessage];

        fn unpin_chat_message<C>(&self, chat_id: C) -> Self::UnpinChatMessage where C: Into<ChatId> {
            let this = self;
            $body!(unpin_chat_message this (chat_id: C))
        }
    };
    (@method leave_chat $body:ident $ty:ident) => {
        type LeaveChat = $ty![LeaveChat];

        fn leave_chat<C>(&self, chat_id: C) -> Self::LeaveChat where C: Into<ChatId> {
            let this = self;
            $body!(leave_chat this (chat_id: C))
        }
    };
    (@method get_chat $body:ident $ty:ident) => {
        type GetChat = $ty![GetChat];

        fn get_chat<C>(&self, chat_id: C) -> Self::GetChat where C: Into<ChatId> {
            let this = self;
            $body!(get_chat this (chat_id: C))
        }
    };
    (@method get_chat_administrators $body:ident $ty:ident) => {
        type GetChatAdministrators = $ty![GetChatAdministrators];

        fn get_chat_administrators<C>(&self, chat_id: C) -> Self::GetChatAdministrators where C: Into<ChatId> {
            let this = self;
            $body!(get_chat_administrators this (chat_id: C))
        }
    };
    (@method get_chat_members_count $body:ident $ty:ident) => {
        type GetChatMembersCount = $ty![GetChatMembersCount];

        fn get_chat_members_count<C>(&self, chat_id: C) -> Self::GetChatMembersCount where C: Into<ChatId> {
            let this = self;
            $body!(get_chat_members_count this (chat_id: C))
        }
    };
    (@method get_chat_member $body:ident $ty:ident) => {
        type GetChatMember = $ty![GetChatMember];

        fn get_chat_member<C>(&self, chat_id: C, user_id: i32) -> Self::GetChatMember where C: Into<ChatId> {
            let this = self;
            $body!(get_chat_member this (chat_id: C, user_id: i32))
        }
    };
    (@method set_chat_sticker_set $body:ident $ty:ident) => {
        type SetChatStickerSet = $ty![SetChatStickerSet];

        fn set_chat_sticker_set<C, S>(&self, chat_id: C, sticker_set_name: S) -> Self::SetChatStickerSet where C: Into<ChatId>,
        S: Into<String> {
            let this = self;
            $body!(set_chat_sticker_set this (chat_id: C, sticker_set_name: S))
        }
    };
    (@method delete_chat_sticker_set $body:ident $ty:ident) => {
        type DeleteChatStickerSet = $ty![DeleteChatStickerSet];

        fn delete_chat_sticker_set<C>(&self, chat_id: C) -> Self::DeleteChatStickerSet where C: Into<ChatId> {
            let this = self;
            $body!(delete_chat_sticker_set this (chat_id: C))
        }
    };
    (@method answer_callback_query $body:ident $ty:ident) => {
        type AnswerCallbackQuery = $ty![AnswerCallbackQuery];

        fn answer_callback_query<C>(&self, callback_query_id: C) -> Self::AnswerCallbackQuery where C: Into<String> {
            let this = self;
            $body!(answer_callback_query this (callback_query_id: C))
        }
    };
    (@method set_my_commands $body:ident $ty:ident) => {
        type SetMyCommands = $ty![SetMyCommands];

        fn set_my_commands<C>(&self, commands: C) -> Self::SetMyCommands where C: IntoIterator<Item = BotCommand> {
            let this = self;
            $body!(set_my_commands this (commands: C))
        }
    };
    (@method get_my_commands $body:ident $ty:ident) => {
        type GetMyCommands = $ty![GetMyCommands];

        fn get_my_commands<>(&self, ) -> Self::GetMyCommands where  {
            let this = self;
            $body!(get_my_commands this ())
        }
    };
    (@method answer_inline_query $body:ident $ty:ident) => {
        type AnswerInlineQuery = $ty![AnswerInlineQuery];

        fn answer_inline_query<I, R>(&self, inline_query_id: I, results: R) -> Self::AnswerInlineQuery where I: Into<String>,
        R: IntoIterator<Item = InlineQueryResult> {
            let this = self;
            $body!(answer_inline_query this (inline_query_id: I, results: R))
        }
    };
    (@method edit_message_text $body:ident $ty:ident) => {
        type EditMessageText = $ty![EditMessageText];

        fn edit_message_text<C, T>(&self, chat_id: C, message_id: i32, text: T) -> Self::EditMessageText where C: Into<ChatId>,
        T: Into<String> {
            let this = self;
            $body!(edit_message_text this (chat_id: C, message_id: i32, text: T))
        }
    };
    (@method edit_message_text_inline $body:ident $ty:ident) => {
        type EditMessageTextInline = $ty![EditMessageTextInline];

        fn edit_message_text_inline<I, T>(&self, inline_message_id: I, text: T) -> Self::EditMessageTextInline where I: Into<String>,
        T: Into<String> {
            let this = self;
            $body!(edit_message_text_inline this (inline_message_id: I, text: T))
        }
    };
    (@method edit_message_caption $body:ident $ty:ident) => {
        type EditMessageCaption = $ty![EditMessageCaption];

        fn edit_message_caption<Ch, Ca>(&self, chat_id: Ch, message_id: i32, caption: Ca) -> Self::EditMessageCaption where Ch: Into<ChatId>,
        Ca: Into<String> {
            let this = self;
            $body!(edit_message_caption this (chat_id: Ch, message_id: i32, caption: Ca))
        }
    };
    (@method edit_message_caption_inline $body:ident $ty:ident) => {
        type EditMessageCaptionInline = $ty![EditMessageCaptionInline];

        fn edit_message_caption_inline<I, C>(&self, inline_message_id: I, caption: C) -> Self::EditMessageCaptionInline where I: Into<String>,
        C: Into<String> {
            let this = self;
            $body!(edit_message_caption_inline this (inline_message_id: I, caption: C))
        }
    };
    (@method edit_message_media $body:ident $ty:ident) => {
        type EditMessageMedia = $ty![EditMessageMedia];

        fn edit_message_media<C>(&self, chat_id: C, message_id: i32, media: InputMedia) -> Self::EditMessageMedia where C: Into<ChatId> {
            let this = self;
            $body!(edit_message_media this (chat_id: C, message_id: i32, media: InputMedia))
        }
    };
    (@method edit_message_media_inline $body:ident $ty:ident) => {
        type EditMessageMediaInline = $ty![EditMessageMediaInline];

        fn edit_message_media_inline<I>(&self, inline_message_id: I, media: InputMedia) -> Self::EditMessageMediaInline where I: Into<String> {
            let this = self;
            $body!(edit_message_media_inline this (inline_message_id: I, media: InputMedia))
        }
    };
    (@method edit_message_reply_markup $body:ident $ty:ident) => {
        type EditMessageReplyMarkup = $ty![EditMessageReplyMarkup];

        fn edit_message_reply_markup<C>(&self, chat_id: C, message_id: i32) -> Self::EditMessageReplyMarkup where C: Into<ChatId> {
            let this = self;
            $body!(edit_message_reply_markup this (chat_id: C, message_id: i32))
        }
    };
    (@method edit_message_reply_markup_inline $body:ident $ty:ident) => {
        type EditMessageReplyMarkupInline = $ty![EditMessageReplyMarkupInline];

        fn edit_message_reply_markup_inline<I>(&self, inline_message_id: I) -> Self::EditMessageReplyMarkupInline where I: Into<String> {
            let this = self;
            $body!(edit_message_reply_markup_inline this (inline_message_id: I))
        }
    };
    (@method stop_poll $body:ident $ty:ident) => {
        type StopPoll = $ty![StopPoll];

        fn stop_poll<C>(&self, chat_id: C, message_id: i32) -> Self::StopPoll where C: Into<ChatId> {
            let this = self;
            $body!(stop_poll this (chat_id: C, message_id: i32))
        }
    };
    (@method delete_message $body:ident $ty:ident) => {
        type DeleteMessage = $ty![DeleteMessage];

        fn delete_message<C>(&self, chat_id: C, message_id: i32) -> Self::DeleteMessage where C: Into<ChatId> {
            let this = self;
            $body!(delete_message this (chat_id: C, message_id: i32))
        }
    };
    (@method send_sticker $body:ident $ty:ident) => {
        type SendSticker = $ty![SendSticker];

        fn send_sticker<C>(&self, chat_id: C, sticker: InputFile) -> Self::SendSticker where C: Into<ChatId> {
            let this = self;
            $body!(send_sticker this (chat_id: C, sticker: InputFile))
        }
    };
    (@method get_sticker_set $body:ident $ty:ident) => {
        type GetStickerSet = $ty![GetStickerSet];

        fn get_sticker_set<N>(&self, name: N) -> Self::GetStickerSet where N: Into<String> {
            let this = self;
            $body!(get_sticker_set this (name: N))
        }
    };
    (@method upload_sticker_file $body:ident $ty:ident) => {
        type UploadStickerFile = $ty![UploadStickerFile];

        fn upload_sticker_file<>(&self, user_id: i32, png_sticker: InputFile) -> Self::UploadStickerFile where  {
            let this = self;
            $body!(upload_sticker_file this (user_id: i32, png_sticker: InputFile))
        }
    };
    (@method create_new_sticker_set $body:ident $ty:ident) => {
        type CreateNewStickerSet = $ty![CreateNewStickerSet];

        fn create_new_sticker_set<N, T, E>(&self, user_id: i32, name: N, title: T, emojis: E) -> Self::CreateNewStickerSet where N: Into<String>,
        T: Into<String>,
        E: Into<String> {
            let this = self;
            $body!(create_new_sticker_set this (user_id: i32, name: N, title: T, emojis: E))
        }
    };
    (@method add_sticker_to_set $body:ident $ty:ident) => {
        type AddStickerToSet = $ty![AddStickerToSet];

        fn add_sticker_to_set<N, E>(&self, user_id: i32, name: N, sticker: InputSticker, emojis: E) -> Self::AddStickerToSet where N: Into<String>,
        E: Into<String> {
            let this = self;
            $body!(add_sticker_to_set this (user_id: i32, name: N, sticker: InputSticker, emojis: E))
        }
    };
    (@method set_sticker_position_in_set $body:ident $ty:ident) => {
        type SetStickerPositionInSet = $ty![SetStickerPositionInSet];

        fn set_sticker_position_in_set<S>(&self, sticker: S, position: u32) -> Self::SetStickerPositionInSet where S: Into<String> {
            let this = self;
            $body!(set_sticker_position_in_set this (sticker: S, position: u32))
        }
    };(@method delete_sticker_from_set $body:ident $ty:ident) => {
        type DeleteStickerFromSet = $ty![DeleteStickerFromSet];

        fn delete_sticker_from_set<S>(&self, sticker: S) -> Self::DeleteStickerFromSet where S: Into<String> {
            let this = self;
            $body!(delete_sticker_from_set this (sticker: S))
        }
    };
    (@method set_sticker_set_thumb $body:ident $ty:ident) => {
        type SetStickerSetThumb = $ty![SetStickerSetThumb];

        fn set_sticker_set_thumb<N>(&self, name: N, user_id: i32) -> Self::SetStickerSetThumb where N: Into<String> {
            let this = self;
            $body!(set_sticker_set_thumb this (name: N, user_id: i32))
        }
    };
    (@method send_invoice $body:ident $ty:ident) => {
        type SendInvoice = $ty![SendInvoice];

        fn send_invoice<T, D, Pa, P, S, C, Pri>(&self, chat_id: i32, title: T, description: D, payload: Pa, provider_token: P, start_parameter: S, currency: C, prices: Pri) -> Self::SendInvoice where T: Into<String>,
        D: Into<String>,
        Pa: Into<String>,
        P: Into<String>,
        S: Into<String>,
        C: Into<String>,
        Pri: IntoIterator<Item = LabeledPrice> {
            let this = self;
            $body!(send_invoice this (chat_id: i32, title: T, description: D, payload: Pa, provider_token: P, start_parameter: S, currency: C, prices: Pri))
        }
    };
    (@method answer_shipping_query $body:ident $ty:ident) => {
        type AnswerShippingQuery = $ty![AnswerShippingQuery];

        fn answer_shipping_query<S>(&self, shipping_query_id: S, ok: bool) -> Self::AnswerShippingQuery where S: Into<String> {
            let this = self;
            $body!(answer_shipping_query this (shipping_query_id: S, ok: bool))
        }
    };
    (@method answer_pre_checkout_query $body:ident $ty:ident) => {
        type AnswerPreCheckoutQuery = $ty![AnswerPreCheckoutQuery];

        fn answer_pre_checkout_query<P>(&self, pre_checkout_query_id: P, ok: bool) -> Self::AnswerPreCheckoutQuery where P: Into<String> {
            let this = self;
            $body!(answer_pre_checkout_query this (pre_checkout_query_id: P, ok: bool))
        }
    };
    (@method set_passport_data_errors $body:ident $ty:ident) => {
        type SetPassportDataErrors = $ty![SetPassportDataErrors];

        fn set_passport_data_errors<E>(&self, user_id: i32, errors: E) -> Self::SetPassportDataErrors where E: IntoIterator<Item = PassportElementError> {
            let this = self;
            $body!(set_passport_data_errors this (user_id: i32, errors: E))
        }
    };
}
