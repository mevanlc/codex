use super::candidate::MentionType;
use crate::file_search::FileSearchScope;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SearchMode {
    Results,
    Filesystem,
    FilesystemAll,
    Tools,
}

impl SearchMode {
    pub(super) fn previous(self) -> Self {
        match self {
            Self::Results => Self::Tools,
            Self::Filesystem => Self::Results,
            Self::FilesystemAll => Self::Filesystem,
            Self::Tools => Self::FilesystemAll,
        }
    }

    pub(super) fn next(self) -> Self {
        match self {
            Self::Results => Self::Filesystem,
            Self::Filesystem => Self::FilesystemAll,
            Self::FilesystemAll => Self::Tools,
            Self::Tools => Self::Results,
        }
    }

    pub(super) fn accepts(self, mention_type: MentionType) -> bool {
        match self {
            Self::Results => true,
            Self::Filesystem | Self::FilesystemAll => {
                matches!(mention_type, MentionType::File | MentionType::Directory)
            }
            Self::Tools => matches!(mention_type, MentionType::Plugin | MentionType::Skill),
        }
    }

    pub(super) fn file_search_scope(self) -> FileSearchScope {
        match self {
            Self::FilesystemAll => FileSearchScope::All,
            Self::Results | Self::Filesystem | Self::Tools => FileSearchScope::Standard,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Results => "All Results",
            Self::Filesystem => "Filesystem",
            Self::FilesystemAll => "Filesystem (All)",
            Self::Tools => "Plugins",
        }
    }
}
