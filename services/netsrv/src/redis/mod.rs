//mod connection;  // Not used anymore
//mod data_fetcher; // Old implementation, not used
mod data_fetcher_new;

//pub use connection::RedisConnection;
//pub use data_fetcher::RedisDataFetcher;
pub use data_fetcher_new::RedisDataFetcher as NewRedisDataFetcher;
