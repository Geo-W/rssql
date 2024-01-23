use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::marker::PhantomData;

use tiberius::{QueryStream, ToSql};
use tokio::net::TcpStream;
use tokio_util::compat::Compat;

use crate::error::custom_error::SsqlResult;
use crate::structs::filter::{ColExpr, FilterExpr};
use crate::structs::ssql_marker::SsqlMarker;
use crate::structs::stream::RowStream;
use crate::structs::JoinArg;

pub struct RawQuery;

pub struct NormalQuery;

pub trait Executable {
    fn execute<'b>(
        &self,
        conn: &'b mut tiberius::Client<Compat<TcpStream>>,
    ) -> impl Future<Output = SsqlResult<QueryStream<'b>>> + Send;
}

impl<'a> Executable for QueryCore<'a, NormalQuery>
where
// T: SsqlMarker + Send + Sync,
{
    fn execute<'b>(
        &self,
        conn: &'b mut tiberius::Client<Compat<TcpStream>>,
    ) -> impl Future<Output = SsqlResult<QueryStream<'b>>> + Send {
        async move {
            let select_fields = self
                .fields
                .iter()
                .map(|(table, fields)| {
                    fields
                        .iter()
                        .map(|field| format!(r#"{}.{} as "{}.{}""#, table, field, table, field))
                        .reduce(|cur, nxt| format!("{},{}", cur, nxt))
                        .unwrap()
                })
                .reduce(|cur, nxt| format!("{},{}", cur, nxt))
                .unwrap();

            let where_clause = self.get_where_clause();
            let order_clause = match self.order.is_empty() {
                true => "".to_string(),
                false => format!("ORDER BY {} ", self.order),
            };

            // let mut stream = conn.simple_query(r#"SELECT ship_to_id as "CUSTOMER_LIST.ship_to_id", ship_to as "CUSTOMER_LIST.ship_to",
            // volume as "CUSTOMER_LIST.volume", container as "CUSTOMER_LIST.container" FROM CUSTOMER_LIST"#).await.unwrap();
            let stream = conn
                .query(
                    format!(
                        "SELECT {} FROM {} {} {where_clause} {order_clause}",
                        select_fields, self.main_table, self.join
                    ),
                    self.query_params.as_slice(),
                )
                .await?;
            Ok(stream)
        }
    }
}

impl<'a> Executable for QueryCore<'a, RawQuery>
where
// Ta: SsqlMarker + Send + Sync,
{
    fn execute<'b>(
        &self,
        conn: &'b mut tiberius::Client<Compat<TcpStream>>,
    ) -> impl Future<Output = SsqlResult<QueryStream<'b>>> + Send {
        async move {
            let stream = conn
                .query(self.raw_sql.as_ref().unwrap(), self.query_params.as_slice())
                .await?;
            Ok(stream)
        }
    }
}

/// Query object generated by [`TableStruct::query()`], for constructing a builder, making a query, etc.
///
/// [`TableStruct::query()`]: trait.SsqlMarker.html#tymethod.query
pub struct QueryCore<'a, Stage = NormalQuery> {
    main_table: &'static str,
    pub(crate) fields: HashMap<&'static str, Vec<&'static str>>,
    pub(crate) filters: Vec<String>,
    pub(crate) join: String,
    tables: HashSet<&'static str>,
    order: String,
    pub(crate) raw_sql: Option<String>,
    relation_func: fn(&str) -> &'static str,
    pub(crate) query_params: Vec<&'a dyn ToSql>,
    query_idx_counter: i32,

    // _marker: Option<PhantomData<T>>,
    _mark2: PhantomData<Stage>,
}

impl<'a, Stage: 'static> QueryCore<'a, Stage>
where
    // T: SsqlMarker + 'static,
    QueryCore<'a, Stage>: Executable,
{
    pub async fn stream<F, Ret>(
        &mut self,
        conn: &'a mut tiberius::Client<Compat<TcpStream>>,
        func: F,
    ) -> SsqlResult<RowStream<'a, Ret>>
    where
        F: 'static + for<'b> Fn(&'b tiberius::Row) -> Ret + Send,
    {
        let query_stream = self.execute(conn).await?;
        Ok(RowStream::new(query_stream, func))
    }
}

impl<'a> QueryCore<'a, NormalQuery>
where
// T: SsqlMarker,
{
    pub(crate) fn new<'b: 'a>(
        fields: (&'static str, Vec<&'static str>),
        func: fn(&str) -> &'static str,
    ) -> QueryCore<'b>
// where
    //     C: SsqlMarker,
    {
        QueryCore {
            main_table: fields.0,
            tables: HashSet::from([fields.0]),
            fields: HashMap::from([fields]),
            filters: vec![],
            join: String::new(),
            relation_func: func,
            raw_sql: None,
            query_params: vec![], // use for filter
            query_idx_counter: 0, // use for filter
            _mark2: PhantomData,

            order: "".to_string(),
        }
    }

    pub fn filter(&mut self, filter_expr: FilterExpr<'a>) -> SsqlResult<()> {
        // self.query_params.push(filter_expr.conditions);
        match self.tables.contains(filter_expr.col.table) {
            true => {
                self.filters
                    .push(filter_expr.to_sql(&mut self.query_idx_counter, &mut self.query_params));
                Ok(())
            }
            false => Err("the filter applies to a table not in this builder".into()),
        }
    }

    pub(crate) fn order_by(&mut self, column: ColExpr, order_asc: bool) -> SsqlResult<()> {
        match self.tables.contains(column.table) {
            true => {
                if !self.order.is_empty() {
                    self.order.push_str(", ")
                }
                self.order.push_str(&column.full_column_name());
                match order_asc {
                    true => self.order.push_str(" ASC"),
                    false => self.order.push_str(" DESC"),
                }
                Ok(())
            }
            false => Err("Try to make order on a table not in this builder".into()),
        }
    }

    pub(crate) fn join<B>(mut self, join_args: JoinArg) -> Self
    where
        B: SsqlMarker,
    {
        let join_type = match join_args {
            JoinArg::Left => "LEFT",
            JoinArg::Right => "RIGHT",
            JoinArg::Outer => "OUTER",
            JoinArg::Inner => "INNER",
        };
        let name = B::table_name();
        let fields = B::fields();
        let relation = self.find_relation(&name);
        self.join
            .push_str(&format!(" {} JOIN {} ", join_type, relation));
        match self.fields.insert(&name, fields) {
            Some(_v) => panic!("table already joined."),
            None => {
                self.tables.insert(name);
            }
        }
        self
    }

    fn find_relation(&self, table: &str) -> &'static str {
        (self.relation_func)(table)
    }

    fn get_where_clause(&self) -> String {
        match self
            .filters
            .iter()
            .cloned()
            .reduce(|cur, nxt| format!("{} AND {}", cur, nxt))
        {
            None => "".to_string(),
            Some(v) => format!(" WHERE {}", v),
        }
    }
}

impl Default for QueryCore<'_, RawQuery> {
    fn default() -> Self {
        QueryCore {
            main_table: "",
            fields: Default::default(),
            filters: vec![],
            join: "".to_string(),
            tables: Default::default(),
            order: "".to_string(),
            raw_sql: None,
            relation_func: |_| "",
            query_params: vec![],
            query_idx_counter: 0,
            _mark2: Default::default(),
        }
    }
}
