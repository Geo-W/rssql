#[allow(non_snake_case)]
#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use tokio::net::TcpStream;
    use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

    use tiberius::{AuthMethod, Client, Config};

    use rssql::prelude::*;


    #[tokio::test]
    async fn it_works2() {
        let mut client = get_client().await;
        let now = std::time::Instant::now();
        let mut query = Fcst::query();
        query.find_all(&mut client).await.unwrap();
        dbg!(now.elapsed());

        let now = std::time::Instant::now();
        let _query = Fcst::query().get_self::<Fcst>(&mut client).await.unwrap();
        dbg!(now.elapsed());
    }

    #[test]
    fn test() {
        let query = Customerlist::query()
            .join::<Test>();
    }

    #[tokio::test]
    async fn insert() {
        let conn = get_client().await;
        let it = vec![Person { id: 5, Email: "a".to_string() }, Person { id: 6, Email: "a".to_string() }].into_iter();
        let a = Person::insert_many(it, conn).await;
        dbg!(&a);
    }


    pub async fn get_client() -> Client<Compat<TcpStream>> {
        rssql::utils::get_client("username", "password", "host", "database").await
    }

    #[derive(ORM, Debug, Default, Serialize, Deserialize)]
    #[rusql(table = CUSTOMER_LIST)]
    pub struct Customerlist {
        pub(crate) ship_to_id: Option<String>,
        #[rusql(foreign_key = "SLOW_MOVING.stock_in_day")]
        pub(crate) ship_to: Option<String>,
        pub(crate) volume: Option<i32>,
        pub(crate) container: Option<String>,
    }

    #[derive(ORM, Debug, Default)]
    #[rusql(table = SLOW_MOVING)]
    pub struct Test {
        pub(crate) stock_in_day: Option<String>,
        pub(crate) total_value: Option<f64>,
        pub(crate) Week: Option<i64>,
        // pub(crate) Generated_Time: Option<NaiveDateTime>,
    }

    #[derive(ORM, Debug, Default)]
    #[rusql(table = Person)]
    pub struct Person {
        pub(crate) id: i32,
        pub(crate) Email: String,
    }


    #[derive(ORM, Debug, Default)]
    #[rusql(table = FORECAST)]
    pub struct Fcst {
        pub(crate) Customer: Option<String>,
        pub(crate) Material: Option<String>,
        Dv: Option<f64>,
        Route: Option<String>,
        TransitTime: Option<String>,
        Plant: Option<String>
    }


    // #[derive(ORM, Debug, Default)]
    // #[rusql(table = SA)]
    // pub struct Sa {
    //     sa_qty: i64,
    //     material: String,
    //     description: String,
    //     eta: String,
    //     vendor: String,
    //     vendor_id: String,
    //     planner: String,
    //     Generated_Time: NaiveDateTime,
    // }
}

