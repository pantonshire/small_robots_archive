use sqlx::FromRow;

#[derive(Copy, Clone, Debug)]
pub(crate) struct RobotKey<'a> {
    pub(crate) robot_number: i32,
    pub(crate) ident: &'a str,
}

impl<'a> RobotKey<'a> {
    pub(crate) fn page_link(&self) -> String {
        format!("/robot/{}/{}", self.robot_number, self.ident)
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct RobotName<'a> {
    pub(crate) prefix: &'a str,
    pub(crate) suffix: &'a str,
    pub(crate) plural: Option<&'a str>,
}

impl<'a> RobotName<'a> {
    pub(crate) fn full_name(self) -> String {
        let len = self.prefix.len()
            + self.suffix.len()
            + self.plural.map(str::len).unwrap_or(0);

        let mut buffer = String::with_capacity(len);

        buffer.push_str(self.prefix);
        buffer.push_str(self.suffix);
        if let Some(plural) = self.plural {
            buffer.push_str(plural);
        }

        buffer
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct RobotImage<'a> {
    pub(crate) file_name: Option<&'a str>,
    pub(crate) orig_alt: Option<&'a str>,
    pub(crate) custom_alt: Option<&'a str>,
}

impl<'a> RobotImage<'a> {
    fn resource_url(self) -> Option<String> {
        const PREFIX: &str = "/robot_images/";

        self.file_name
            .map(|file_name| {
                let mut buffer = String::with_capacity(PREFIX.len() + file_name.len());
                buffer.push_str(PREFIX);
                buffer.push_str(file_name);
                buffer
            })
    }

    fn alt(self) -> &'a str {
        const MISSING_ALT: &str =
            "Sorry, no alt text was found for this robot. Please direct message me @PantonshireDev on \
            Twitter, and I'll fix it as soon as I can.";

        self.custom_alt
            .or(self.orig_alt)
            .unwrap_or(MISSING_ALT)
    }
}

pub(crate) trait Linkable {
    fn key(&self) -> RobotKey<'_>;

    fn page_link(&self) -> String {
        self.key().page_link()
    }
}

pub(crate) trait Named {
    fn name(&self) -> RobotName<'_>;

    fn full_name(&self) -> String {
        self.name().full_name()
    }
}

pub(crate) trait Displayable {
    fn image(&self) -> RobotImage<'_>;

    fn image_resource_url(&self) -> Option<String> {
        self.image().resource_url()
    }

    fn image_alt(&self) -> &str {
        self.image().alt()
    }
}

#[derive(FromRow, Clone, Debug)]
pub(crate) struct RobotTextLink {
    pub(crate) id: i32,
    pub(crate) robot_number: i32,
    pub(crate) ident: String,
    pub(crate) prefix: String,
    pub(crate) suffix: String,
    pub(crate) plural: Option<String>,
    pub(crate) content_warning: Option<String>,
}

impl Linkable for RobotTextLink {
    fn key(&self) -> RobotKey<'_> {
        RobotKey {
            robot_number: self.robot_number,
            ident: &self.ident,
        }
    }
}

impl Named for RobotTextLink {
    fn name(&self) -> RobotName<'_> {
        RobotName {
            prefix: &self.prefix,
            suffix: &self.suffix,
            plural: self.plural.as_deref(),
        }
    }
}

#[derive(FromRow, Clone, Debug)]
pub(crate) struct RobotPreview {
    pub(crate) id: i32,
    pub(crate) robot_number: i32,
    pub(crate) ident: String,
    pub(crate) prefix: String,
    pub(crate) suffix: String,
    pub(crate) plural: Option<String>,
    pub(crate) content_warning: Option<String>,
    pub(crate) image_thumb_path: Option<String>,
    pub(crate) alt: Option<String>,
    pub(crate) custom_alt: Option<String>,
}

impl Linkable for RobotPreview {
    fn key(&self) -> RobotKey<'_> {
        RobotKey {
            robot_number: self.robot_number,
            ident: &self.ident,
        }
    }
}

impl Named for RobotPreview {
    fn name(&self) -> RobotName<'_> {
        RobotName {
            prefix: &self.prefix,
            suffix: &self.suffix,
            plural: self.plural.as_deref(),
        }
    }
}

impl Displayable for RobotPreview {
    fn image(&self) -> RobotImage<'_> {
        RobotImage {
            file_name: self.image_thumb_path.as_deref(),
            orig_alt: self.alt.as_deref(),
            custom_alt: self.custom_alt.as_deref(),
        }
    }
}

#[derive(FromRow, Clone, Debug)]
pub(crate) struct RobotFull {
    pub(crate) id: i32,
    pub(crate) robot_number: i32,
    pub(crate) ident: String,
    pub(crate) prefix: String,
    pub(crate) suffix: String,
    pub(crate) plural: Option<String>,
    pub(crate) content_warning: Option<String>,
    pub(crate) image_path: Option<String>,
    pub(crate) alt: Option<String>,
    pub(crate) custom_alt: Option<String>,
    pub(crate) body: String,
    pub(crate) tweet_id: i64,
}

impl Linkable for RobotFull {
    fn key(&self) -> RobotKey<'_> {
        RobotKey {
            robot_number: self.robot_number,
            ident: &self.ident,
        }
    }
}

impl Named for RobotFull {
    fn name(&self) -> RobotName<'_> {
        RobotName {
            prefix: &self.prefix,
            suffix: &self.suffix,
            plural: self.plural.as_deref(),
        }
    }
}

impl Displayable for RobotFull {
    fn image(&self) -> RobotImage<'_> {
        RobotImage {
            file_name: self.image_path.as_deref(),
            orig_alt: self.alt.as_deref(),
            custom_alt: self.custom_alt.as_deref(),
        }
    }
}

#[derive(FromRow, Copy, Clone, Debug)]
pub(crate) struct Count {
    pub(crate) count: i64,
}

impl Count {
    pub(crate) fn pages(self, page_size: u32) -> u32 {
        (((self.count.max(0) - 1) / (page_size as i64)) + 1) as u32
    }
}
