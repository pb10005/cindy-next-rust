use async_graphql::{self, guard::Guard, Context, InputObject, Object, Subscription};
use chrono::{Duration, Utc};
use diesel::prelude::*;
use futures::{Stream, StreamExt};

use crate::auth::Role;
use crate::broker::CindyBroker;
use crate::context::{GlobalCtx, RequestCtx};
use crate::models::puzzle::*;
use crate::models::*;
use crate::schema::puzzle;

#[derive(Default)]
pub struct PuzzleQuery;
#[derive(Default)]
pub struct PuzzleMutation;
#[derive(Default)]
pub struct PuzzleSubscription;

#[Object]
impl PuzzleQuery {
    pub async fn puzzle(&self, ctx: &Context<'_>, id: i32) -> async_graphql::Result<Puzzle> {
        let conn = ctx.data::<GlobalCtx>()?.get_conn()?;

        let puzzle = puzzle::table
            .filter(puzzle::id.eq(id))
            .limit(1)
            .first(&conn)?;

        Ok(puzzle)
    }

    pub async fn puzzles(
        &self,
        ctx: &Context<'_>,
        limit: Option<i64>,
        offset: Option<i64>,
        filter: Option<Vec<PuzzleFilter>>,
        order: Option<Vec<PuzzleOrder>>,
    ) -> async_graphql::Result<Vec<Puzzle>> {
        use crate::schema::puzzle::dsl::*;

        let conn = ctx.data::<GlobalCtx>()?.get_conn()?;

        let mut query = puzzle.into_boxed();
        if let Some(order) = order {
            query = PuzzleOrders::new(order).apply_order(query);
        }
        if let Some(filter) = filter {
            if let Some(filter_exp) = filter.as_expression() {
                query = query.filter(filter_exp)
            }
        }
        if let Some(limit) = limit {
            query = query.limit(limit);
        }
        if let Some(offset) = offset {
            query = query.offset(offset);
        }

        let puzzles = query.load::<Puzzle>(&conn)?;

        Ok(puzzles)
    }

    pub async fn puzzle_count(
        &self,
        ctx: &Context<'_>,
        filter: Option<Vec<PuzzleFilter>>,
    ) -> async_graphql::Result<i64> {
        use crate::schema::puzzle::dsl::*;

        let conn = ctx.data::<GlobalCtx>()?.get_conn()?;

        let mut query = puzzle.into_boxed();
        if let Some(filter) = filter {
            if let Some(filter_exp) = filter.as_expression() {
                query = query.filter(filter_exp)
            }
        }

        let result = query.count().get_result(&conn)?;

        Ok(result)
    }
}

#[derive(InputObject)]
pub struct UpdatePuzzleInput {
    pub title: Option<String>,
    pub yami: Option<Yami>,
    pub genre: Option<Genre>,
    pub content: Option<String>,
    pub solution: Option<String>,
    pub created: Option<Timestamptz>,
    pub modified: Option<Timestamptz>,
    pub status: Option<Status>,
    pub memo: Option<String>,
    pub user_id: Option<i32>,
    pub anonymous: Option<bool>,
    pub dazed_on: Option<Date>,
    pub grotesque: Option<bool>,
}

#[derive(AsChangeset, Debug)]
#[table_name = "puzzle"]
pub struct UpdatePuzzleData {
    pub title: Option<String>,
    pub yami: Option<i32>,
    pub genre: Option<i32>,
    pub content: Option<String>,
    pub solution: Option<String>,
    pub created: Option<Timestamptz>,
    pub modified: Option<Timestamptz>,
    pub status: Option<i32>,
    pub memo: Option<String>,
    pub user_id: Option<i32>,
    pub anonymous: Option<bool>,
    pub dazed_on: Option<Date>,
    pub grotesque: Option<bool>,
}

impl From<UpdatePuzzleInput> for UpdatePuzzleData {
    fn from(data: UpdatePuzzleInput) -> Self {
        Self {
            title: data.title,
            yami: data.yami.map(|yami| yami as i32),
            genre: data.yami.map(|genre| genre as i32),
            content: data.content,
            solution: data.solution,
            created: data.created,
            modified: data.modified,
            status: data.status.map(|status| status as i32),
            memo: data.memo,
            user_id: data.user_id,
            anonymous: data.anonymous,
            dazed_on: data.dazed_on,
            grotesque: data.grotesque,
        }
    }
}

/// Calculate dazing duration of a puzzle
#[derive(Default)]
struct DazedTimeCalc {
    yami: Option<Yami>,
    genre: Option<Genre>,
}

impl DazedTimeCalc {
    pub fn yami(mut self, yami: Option<Yami>) -> Self {
        self.yami = yami;
        self
    }
    pub fn genre(mut self, genre: Option<Genre>) -> Self {
        self.genre = genre;
        self
    }
    /// Get dazing duration
    pub fn duration(&self) -> Duration {
        dotenv::dotenv().ok();
        let mut duration = std::env::var("DAZE_DURATION_DEFAULT").unwrap_or("7".to_owned());

        if let Some(genre) = self.genre {
            match genre {
                Genre::Classic => {
                    if let Ok(value) = std::env::var("DAZE_DURATION_GENRE_CLASSIC") {
                        duration = value;
                    }
                }
                Genre::TwentyQuestions => {
                    if let Ok(value) = std::env::var("DAZE_DURATION_GENRE_TWENTY_QUESTIONS") {
                        duration = value;
                    }
                }
                Genre::LittleAlbat => {
                    if let Ok(value) = std::env::var("DAZE_DURATION_GENRE_LITTLE_ALBAT") {
                        duration = value;
                    }
                }
                Genre::Others => {
                    if let Ok(value) = std::env::var("DAZE_DURATION_GENRE_OTHERS") {
                        duration = value;
                    }
                }
            }
        }

        if let Some(yami) = self.yami {
            match yami {
                Yami::None => {
                    if let Ok(value) = std::env::var("DAZE_DURATION_YAMI_NONE") {
                        duration = value;
                    }
                }
                Yami::Normal => {
                    if let Ok(value) = std::env::var("DAZE_DURATION_YAMI_NORMAL") {
                        duration = value;
                    }
                }
                Yami::Longterm => {
                    if let Ok(value) = std::env::var("DAZE_DURATION_YAMI_LONGTERM") {
                        duration = value;
                    }
                }
            }
        }

        Duration::days(
            duration
                .parse::<i64>()
                .expect("Invalid DAZE_DURATION_* variable"),
        )
    }
}

#[derive(InputObject)]
pub struct CreatePuzzleInput {
    pub title: Option<String>,
    pub yami: Option<Yami>,
    pub genre: Option<Genre>,
    pub content: Option<String>,
    pub solution: Option<String>,
    pub created: Option<Timestamptz>,
    pub modified: Option<Timestamptz>,
    pub status: Option<Status>,
    pub memo: Option<String>,
    pub user_id: Option<i32>,
    pub anonymous: Option<bool>,
    pub dazed_on: Option<Date>,
    pub grotesque: Option<bool>,
}

impl CreatePuzzleInput {
    pub fn set_default(mut self) -> Self {
        let now = Utc::now();
        // Set field `created`
        if self.created.is_none() {
            self.created = Some(now.clone());
        };

        // Set field `dazed_on`
        if self.dazed_on.is_none() {
            self.dazed_on = Some(
                now.date().naive_utc()
                    + DazedTimeCalc::default()
                        .yami(self.yami.clone())
                        .genre(self.genre.clone())
                        .duration(),
            );
        };

        // Set field `status`
        if self.status.is_none() {
            self.status = Some(Status::Undergoing);
        };

        self
    }

    pub fn set_user_id(mut self, user_id: Option<i32>) -> Self {
        self.user_id = user_id;
        self
    }
}

#[derive(Insertable)]
#[table_name = "puzzle"]
pub struct CreatePuzzleData {
    pub title: Option<String>,
    pub yami: Option<i32>,
    pub genre: Option<i32>,
    pub content: Option<String>,
    pub solution: Option<String>,
    pub created: Option<Timestamptz>,
    pub modified: Option<Timestamptz>,
    pub status: Option<i32>,
    pub memo: Option<String>,
    pub user_id: Option<i32>,
    pub anonymous: Option<bool>,
    pub dazed_on: Option<Date>,
    pub grotesque: Option<bool>,
}

impl From<CreatePuzzleInput> for CreatePuzzleData {
    fn from(data: CreatePuzzleInput) -> Self {
        Self {
            title: data.title,
            yami: data.yami.map(|yami| yami as i32),
            genre: data.yami.map(|genre| genre as i32),
            content: data.content,
            solution: data.solution,
            created: data.created,
            modified: data.modified,
            status: data.status.map(|status| status as i32),
            memo: data.memo,
            user_id: data.user_id,
            anonymous: data.anonymous,
            dazed_on: data.dazed_on,
            grotesque: data.grotesque,
        }
    }
}

#[Object]
impl PuzzleMutation {
    pub async fn update_puzzle(
        &self,
        ctx: &Context<'_>,
        id: ID,
        mut set: UpdatePuzzleInput,
    ) -> async_graphql::Result<Puzzle> {
        let conn = ctx.data::<GlobalCtx>()?.get_conn()?;

        // User should be the owner on update mutation
        let puzzle_inst: Puzzle = puzzle::table
            .filter(puzzle::id.eq(id))
            .limit(1)
            .first(&conn)?;
        user_id_guard(ctx, puzzle_inst.user_id)?;

        // Prevent further edit from user if its status is forced hidden
        if let Status::ForceHidden = puzzle_inst.status {
            return Err(async_graphql::Error::new(
                "Further edits are blocked from a forced hidden puzzle",
            ));
        };

        // Set `modified` to the current time when puzzle is solved
        // TODO rename `modified` -> `time_solved`
        if puzzle_inst.status == Status::Undergoing && set.status != Some(Status::Undergoing) {
            set.modified = Some(Utc::now());
        };

        let puzzle: Puzzle = diesel::update(puzzle::table)
            .filter(puzzle::id.eq(id))
            .set(UpdatePuzzleData::from(set))
            .get_result(&conn)
            .map_err(|err| async_graphql::Error::from(err))?;

        CindyBroker::publish(PuzzleSub::Updated(puzzle_inst, puzzle.clone()));

        Ok(puzzle)
    }

    pub async fn create_puzzle(
        &self,
        ctx: &Context<'_>,
        data: CreatePuzzleInput,
    ) -> async_graphql::Result<Puzzle> {
        let conn = ctx.data::<GlobalCtx>()?.get_conn()?;
        let reqctx = ctx.data::<RequestCtx>()?;
        let user_id = reqctx.get_user_id();
        let role = reqctx.get_role();

        let insert_data = match role {
            Role::User => {
                // Assert that time-related are unset
                assert_eq_guard(data.created, None)?;
                assert_eq_guard(data.modified, None)?;
                // Assert user_id is set to the user
                let insert_data = if let Some(user_id) = data.user_id {
                    user_id_guard(ctx, user_id)?;
                    CreatePuzzleData::from(data.set_default())
                } else {
                    CreatePuzzleData::from(data.set_default().set_user_id(user_id))
                };

                insert_data
            }
            Role::Admin => CreatePuzzleData::from(data.set_default()),
            Role::Guest => return Err(async_graphql::Error::new("User not logged in")),
        };

        let puzzle: Puzzle = diesel::insert_into(puzzle::table)
            .values(&insert_data)
            .get_result(&conn)
            .map_err(|err| async_graphql::Error::from(err))?;

        CindyBroker::publish(PuzzleSub::Created(puzzle.clone()));

        Ok(puzzle)
    }

    // Delete puzzle (admin only)
    #[graphql(guard(and(
        DenyRoleGuard(role = "Role::User"),
        DenyRoleGuard(role = "Role::Guest")
    )))]
    pub async fn delete_puzzle(&self, ctx: &Context<'_>, id: ID) -> async_graphql::Result<Puzzle> {
        let conn = ctx.data::<GlobalCtx>()?.get_conn()?;

        let puzzle = diesel::delete(puzzle::table.filter(puzzle::id.eq(id)))
            .get_result(&conn)
            .map_err(|err| async_graphql::Error::from(err))?;

        Ok(puzzle)
    }
}

#[derive(InputObject, Eq, PartialEq, Clone)]
pub struct PuzzleSubFilter {
    id: Option<I32Filtering>,
    status: Option<StatusFiltering>,
    yami: Option<YamiFiltering>,
    genre: Option<GenreFiltering>,
}

impl RawFilter<Puzzle> for PuzzleSubFilter {
    fn check(&self, item: &Puzzle) -> bool {
        if let Some(filter) = self.id.as_ref() {
            filter.check(&item.id)
        } else if let Some(filter) = self.status.as_ref() {
            filter.check(&item.status)
        } else if let Some(filter) = self.yami.as_ref() {
            filter.check(&item.yami)
        } else if let Some(filter) = self.genre.as_ref() {
            filter.check(&item.genre)
        } else {
            true
        }
    }
}

#[Subscription]
impl PuzzleSubscription {
    pub async fn puzzle_sub(
        &self,
        filter: Option<PuzzleSubFilter>,
    ) -> impl Stream<Item = Option<PuzzleSub>> {
        CindyBroker::<PuzzleSub>::subscribe().filter(move |puzzle_sub| {
            let check = if let Some(filter) = filter.as_ref() {
                match puzzle_sub {
                    Some(PuzzleSub::Created(puzzle)) => filter.check(&puzzle),
                    Some(PuzzleSub::Updated(orig, _)) => filter.check(&orig),
                    None => false,
                }
            } else {
                puzzle_sub.is_some()
            };

            async move { check }
        })
    }
}
