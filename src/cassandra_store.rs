use cdrs_tokio::cluster::session::{Session, SessionBuilder, TcpSessionBuilder};

use cdrs_tokio::cluster::TcpConnectionManager;
use cdrs_tokio::load_balancing::RoundRobinLoadBalancingStrategy;
use cdrs_tokio::query::*;
use cdrs_tokio::query_values;
use cdrs_tokio::transport::TransportTcp;
use chrono::Utc;
use std::error::Error;
use std::os::unix::net::SocketAddr;
use std::sync::Arc;

use crate::mqttdecoder;

pub type CurrentSession = Session<
    TransportTcp,
    TcpConnectionManager,
    RoundRobinLoadBalancingStrategy<TransportTcp, TcpConnectionManager>,
>;

pub struct CassandraStore {}

type CassandraResult<T> = Result<T, Box<dyn Error>>;

impl CassandraStore {
    pub async fn initialize(session: Arc<CurrentSession>) -> CassandraResult<()> {
        let create_ks = "CREATE KEYSPACE IF NOT EXISTS retain_kv WITH REPLICATION = \
            { 'class' : 'SimpleStrategy', 'replication_factor' : 1 };";

        let _result = session
            .query(create_ks)
            .await
            .expect("Keyspace creation error");

        let create_retain_table_cql = "CREATE TABLE IF NOT EXISTS retain_kv.device_data \
            (device_id text, topic text, payload blob, updatetime timestamp, PRIMARY KEY (device_id, topic));";

        let _result = session
            .query(create_retain_table_cql)
            .await
            .expect("Table creation error");

        Ok(())
    }

    pub async fn store_published_data(
        session: Arc<CurrentSession>,
        packet: mqttdecoder::Publish,
    ) -> CassandraResult<()> {
        let update_struct_cql = "UPDATE retain_kv.device_data \
            SET payload = ?, updatetime = ? WHERE device_id = ? AND topic = ?;";

        session
            .query_with_values(
                update_struct_cql,
                query_values!("device_id" => "testid", "topic" => packet.topic_name, "payload" => packet.payload, "updatetime" => Utc::now().timestamp_millis()),
            )
            .await
            .expect("update");
        Ok(())
    }
}
