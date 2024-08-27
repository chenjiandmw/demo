use crate::entities::{prelude::*, users};

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, ConnectOptions, Database,
    DatabaseBackend, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QuerySelect, QueryTrait,
};

async fn sea_orm_demo() -> DatabaseConnection {
    let opt = ConnectOptions::new("mysql://root:asd123@localhost:8459/test");
    println!("opt:{:?}", opt);
    Database::connect(opt).await.unwrap()
}

pub async fn run() -> Result<(), DbErr> {
    let db_conn = sea_orm_demo().await;
    let r: Option<users::Model> = Users::find_by_id(1).one(&db_conn).await?;
    let mut user: users::ActiveModel;
    // let abc: abc::Model;
    match &r {
        Some(result) => {
            user = result.clone().into();
            user.username = ActiveValue::set(Some(format!("{}-", user.username.unwrap().unwrap())));
            user.id = ActiveValue::Set(3);
            user.bj = ActiveValue::Set(None);
        }
        None => {
            user = users::ActiveModel {
                username: ActiveValue::Set(Some("123".to_string())),
                ..Default::default()
            };
        }
    };
    println!("user:{:?}", r);
    let r = user.save(&db_conn).await?;
    println!("save result:{:?}", r);

    let a = Users::find()
        .select_only()
        .column(users::Column::Id)
        .filter(Condition::any().add(Condition::all().add(users::Column::Bj.lte(10))))
        .build(DatabaseBackend::MySql)
        .to_string();

    println!("自定义查找语句:{:?}", a);

    Ok(())
}
