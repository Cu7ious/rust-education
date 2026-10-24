-- Initial platform schema: accounts, versioned content projection, and learning activity.

CREATE TABLE users (
    id uuid PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE user_identities (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider text NOT NULL CHECK (provider IN ('password', 'google', 'github')),
    provider_subject text NOT NULL,
    identifier_kind text CHECK (identifier_kind IN ('email', 'phone')),
    identifier text,
    email_verified_at timestamptz,
    phone_verified_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT user_identities_provider_subject_unique UNIQUE (provider, provider_subject),
    CONSTRAINT user_identities_password_identifier_check CHECK (
        (
            provider = 'password'
            AND identifier_kind IS NOT NULL
            AND identifier IS NOT NULL
        )
        OR (
            provider <> 'password'
            AND identifier_kind IS NULL
            AND identifier IS NULL
        )
    )
);

CREATE UNIQUE INDEX user_identities_identifier_unique
    ON user_identities (identifier_kind, identifier)
    WHERE identifier IS NOT NULL;

CREATE INDEX user_identities_user_id_idx ON user_identities (user_id);

CREATE TABLE password_credentials (
    identity_id uuid PRIMARY KEY REFERENCES user_identities (id) ON DELETE CASCADE,
    password_hash text NOT NULL,
    password_updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE sessions (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash text NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    user_agent text,
    ip text
);

CREATE INDEX sessions_user_id_idx ON sessions (user_id);

CREATE TABLE verification_tokens (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    identity_id uuid REFERENCES user_identities (id) ON DELETE CASCADE,
    purpose text NOT NULL CHECK (
        purpose IN ('verify_email', 'verify_phone', 'reset_password', 'link_identity')
    ),
    token_hash text NOT NULL UNIQUE,
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE content_releases (
    id uuid PRIMARY KEY,
    course_id text NOT NULL CHECK (course_id ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    revision integer NOT NULL CHECK (revision >= 1),
    manifest_hash text NOT NULL,
    snapshot jsonb NOT NULL,
    published_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (course_id, revision)
);

CREATE TABLE courses (
    id text PRIMARY KEY CHECK (id ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    current_release_id uuid NOT NULL REFERENCES content_releases (id) ON DELETE RESTRICT,
    default_locale text NOT NULL,
    retired_at timestamptz
);

CREATE TABLE course_localizations (
    course_id text NOT NULL REFERENCES courses (id) ON DELETE RESTRICT,
    locale text NOT NULL,
    title text NOT NULL,
    description text NOT NULL DEFAULT '',
    PRIMARY KEY (course_id, locale)
);

CREATE TABLE topics (
    id text PRIMARY KEY CHECK (id ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    course_id text NOT NULL REFERENCES courses (id) ON DELETE RESTRICT,
    position integer NOT NULL CHECK (position <> 0),
    retired_at timestamptz
);

CREATE UNIQUE INDEX topics_course_position_active_idx
    ON topics (course_id, position)
    WHERE retired_at IS NULL;

CREATE INDEX topics_course_id_idx ON topics (course_id);

CREATE TABLE topic_localizations (
    topic_id text NOT NULL REFERENCES topics (id) ON DELETE RESTRICT,
    locale text NOT NULL,
    title text NOT NULL,
    description text NOT NULL DEFAULT '',
    PRIMARY KEY (topic_id, locale)
);

CREATE TABLE subtopics (
    id text PRIMARY KEY CHECK (id ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    topic_id text NOT NULL REFERENCES topics (id) ON DELETE RESTRICT,
    position integer NOT NULL CHECK (position <> 0),
    retired_at timestamptz
);

CREATE UNIQUE INDEX subtopics_topic_position_active_idx
    ON subtopics (topic_id, position)
    WHERE retired_at IS NULL;

CREATE INDEX subtopics_topic_id_idx ON subtopics (topic_id);

CREATE TABLE subtopic_localizations (
    subtopic_id text NOT NULL REFERENCES subtopics (id) ON DELETE RESTRICT,
    locale text NOT NULL,
    title text NOT NULL,
    description text NOT NULL DEFAULT '',
    PRIMARY KEY (subtopic_id, locale)
);

CREATE TABLE lessons (
    id text PRIMARY KEY CHECK (id ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    subtopic_id text NOT NULL REFERENCES subtopics (id) ON DELETE RESTRICT,
    position integer NOT NULL CHECK (position <> 0),
    retired_at timestamptz
);

CREATE UNIQUE INDEX lessons_subtopic_position_active_idx
    ON lessons (subtopic_id, position)
    WHERE retired_at IS NULL;

CREATE INDEX lessons_subtopic_id_idx ON lessons (subtopic_id);

CREATE TABLE lesson_localizations (
    lesson_id text NOT NULL REFERENCES lessons (id) ON DELETE RESTRICT,
    locale text NOT NULL,
    title text NOT NULL,
    description text NOT NULL DEFAULT '',
    PRIMARY KEY (lesson_id, locale)
);

CREATE TABLE blocks (
    id text PRIMARY KEY CHECK (id ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    lesson_id text NOT NULL REFERENCES lessons (id) ON DELETE RESTRICT,
    position integer NOT NULL CHECK (position <> 0),
    kind text NOT NULL CHECK (kind IN ('text', 'video', 'task')),
    text_markdown jsonb,
    video_url text,
    video_title jsonb,
    retired_at timestamptz,
    CONSTRAINT blocks_payload_check CHECK (
        (
            kind = 'text'
            AND text_markdown IS NOT NULL
            AND video_url IS NULL
            AND video_title IS NULL
        )
        OR (
            kind = 'video'
            AND text_markdown IS NULL
            AND video_url IS NOT NULL
            AND video_title IS NOT NULL
        )
        OR (
            kind = 'task'
            AND text_markdown IS NULL
            AND video_url IS NULL
            AND video_title IS NULL
        )
    )
);

CREATE UNIQUE INDEX blocks_lesson_position_active_idx
    ON blocks (lesson_id, position)
    WHERE retired_at IS NULL;

CREATE INDEX blocks_lesson_id_idx ON blocks (lesson_id);

CREATE TABLE tasks (
    id text PRIMARY KEY CHECK (id ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    block_id text UNIQUE REFERENCES blocks (id) ON DELETE RESTRICT,
    course_id text NOT NULL REFERENCES courses (id) ON DELETE RESTRICT,
    task_type text NOT NULL CHECK (task_type IN ('single_choice', 'multiple_choice', 'short_answer')),
    purpose text NOT NULL CHECK (purpose IN ('lesson', 'assessment', 'both')),
    grading_hash text NOT NULL,
    public_payload jsonb NOT NULL,
    retired_at timestamptz
);

CREATE INDEX tasks_course_id_idx ON tasks (course_id);

CREATE TABLE task_grading (
    task_id text PRIMARY KEY REFERENCES tasks (id) ON DELETE RESTRICT,
    grading_hash text NOT NULL,
    spec jsonb NOT NULL
);

CREATE TABLE assessments (
    id text PRIMARY KEY CHECK (id ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    scope text NOT NULL CHECK (scope IN ('subtopic', 'course')),
    scope_id text NOT NULL,
    course_id text NOT NULL REFERENCES courses (id) ON DELETE RESTRICT,
    pass_percent integer NOT NULL DEFAULT 80 CHECK (pass_percent BETWEEN 1 AND 100),
    retired_at timestamptz,
    UNIQUE (scope, scope_id)
);

CREATE INDEX assessments_course_id_idx ON assessments (course_id);

CREATE TABLE assessment_items (
    assessment_id text NOT NULL REFERENCES assessments (id) ON DELETE RESTRICT,
    position integer NOT NULL CHECK (position > 0),
    task_id text NOT NULL REFERENCES tasks (id) ON DELETE RESTRICT,
    PRIMARY KEY (assessment_id, position)
);

CREATE INDEX assessment_items_task_id_idx ON assessment_items (task_id);

CREATE TABLE task_attempts (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    task_id text NOT NULL REFERENCES tasks (id) ON DELETE RESTRICT,
    release_id uuid NOT NULL REFERENCES content_releases (id) ON DELETE RESTRICT,
    idempotency_key uuid NOT NULL,
    submitted_answer jsonb NOT NULL,
    correct boolean NOT NULL,
    feedback jsonb NOT NULL,
    correct_choice_ids jsonb,
    context text NOT NULL CHECK (context IN ('lesson', 'subtopic_test', 'course_test', 'trainer')),
    context_id text,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (user_id, idempotency_key)
);

CREATE INDEX task_attempts_user_task_created_idx
    ON task_attempts (user_id, task_id, created_at DESC);

CREATE TABLE lesson_progress (
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    lesson_id text NOT NULL REFERENCES lessons (id) ON DELETE RESTRICT,
    status text NOT NULL CHECK (status IN ('not_started', 'in_progress', 'completed')),
    completed_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, lesson_id)
);

CREATE TABLE question_mastery (
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    task_id text NOT NULL REFERENCES tasks (id) ON DELETE RESTRICT,
    state text NOT NULL CHECK (state IN ('unseen', 'learning', 'good', 'easy', 'needs_review')),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, task_id)
);

CREATE INDEX question_mastery_user_state_idx ON question_mastery (user_id, state);

CREATE TABLE assessment_attempts (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    assessment_id text NOT NULL REFERENCES assessments (id) ON DELETE RESTRICT,
    release_id uuid NOT NULL REFERENCES content_releases (id) ON DELETE RESTRICT,
    idempotency_key uuid NOT NULL,
    score_percent numeric(5, 2) NOT NULL,
    passed boolean NOT NULL,
    started_at timestamptz NOT NULL DEFAULT now(),
    submitted_at timestamptz,
    UNIQUE (user_id, idempotency_key)
);

CREATE TABLE assessment_attempt_answers (
    attempt_id uuid NOT NULL REFERENCES assessment_attempts (id) ON DELETE CASCADE,
    task_id text NOT NULL REFERENCES tasks (id) ON DELETE RESTRICT,
    task_attempt_id uuid NOT NULL REFERENCES task_attempts (id) ON DELETE CASCADE,
    PRIMARY KEY (attempt_id, task_id)
);
