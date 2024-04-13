use crate::mqttcoder;
use itertools::Itertools;
use std::collections::HashMap;
use std::fmt::Debug;
use tokio::sync::mpsc;
use tracing;
use tracing::{debug, trace, warn};

#[derive(Debug)]
pub struct TopicFilterStore<T> {
    elements: HashMap<String, Vec<T>>,
}

#[derive(Debug)]
pub struct SubInfo {
    pub topicfilter_elements: Vec<String>,
    pub sender: Option<mpsc::Sender<mqttcoder::MQTTPacket>>,
    pub client_id: String,
}

impl TopicFilter for SubInfo {
    fn get_topic_filter(&self) -> &Vec<String> {
        return &self.topicfilter_elements;
    }
}

impl SubInfo {
    pub fn new(
        topicfilter: String,
        sender: Option<mpsc::Sender<mqttcoder::MQTTPacket>>,
        client_id: String,
    ) -> Self {
        SubInfo {
            topicfilter_elements: topicfilter.split('/').map(|s| s.to_string()).collect(),
            sender,
            client_id,
        }
    }
}
pub trait TopicFilter {
    fn get_topic_filter(&self) -> &Vec<String>;
}

impl<T: TopicFilter + Debug> TopicFilterStore<T> {
    pub fn new() -> Self {
        return TopicFilterStore {
            elements: HashMap::new(),
        };
    }

    #[tracing::instrument(level = "trace")]
    pub fn remove_subscription(&mut self, id: &String) {
        self.elements.remove(id);
    }

    #[tracing::instrument(level = "trace")]
    pub fn register_topicfilter(&mut self, topicfilter: T, id: String) -> std::io::Result<()> {
        if topicfilter
            .get_topic_filter()
            .iter()
            .any(|s| !self.valid(s))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Invalid Topicfilter",
            ));
        }
        if let Some(v) = self.elements.get_mut(&id) {
            v.push(topicfilter);
        } else {
            self.elements.insert(id, vec![topicfilter]);
        }
        Ok(())
    }

    #[tracing::instrument(level = "trace")]
    pub fn get_topicfilter(&mut self, topic: &String) -> std::io::Result<Vec<&T>> {
        let mut ret = vec![];
        trace!(
            "get topic filter {:?} vs list {:?}",
            topic,
            &self.elements.len()
        );
        for (_, elements) in &self.elements {
            for topic_filter in elements {
                let mut matched = true;

                for eob in topic
                    .split('/')
                    .into_iter()
                    .zip_longest(topic_filter.get_topic_filter())
                {
                    match eob {
                        itertools::EitherOrBoth::Both(te, tfe) => {
                            if tfe == "#" {
                                matched = true;
                                break;
                            }
                            if te != tfe && tfe != "+" {
                                // end
                                matched = false;
                                break;
                            }
                        }
                        itertools::EitherOrBoth::Left(_) => {
                            // end
                            // topic: "hoge/fuga" filter: "hoge/" -> not match
                            matched = false;
                            break;
                        }
                        itertools::EitherOrBoth::Right(_) => {
                            // end
                            matched = false;
                            break;
                        }
                    }
                }
                if matched {
                    debug!("topic matched, so push {:?}", topic_filter);
                    ret.push(topic_filter);
                }
                // check next filter
            }
        }

        return Ok(ret);
    }

    #[tracing::instrument(level = "trace")]
    fn valid(&self, element: &str) -> bool {
        if "#" == element || "+" == element || "" == element {
            return true;
        }
        for c in element.chars() {
            // A-Za-z0-9
            if !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '.' {
                return false;
            }
        }
        return true;
    }
}

#[cfg(test)]
mod tests {
    // for internal access
    use super::*;

    #[test]
    fn test_check_element() {
        let m = TopicFilterStore::<SubInfo>::new();
        assert_eq!(m.valid("helloworld"), true);
        assert_eq!(m.valid("hello-world"), true);
        assert_eq!(m.valid("hello-worl.d"), true);
        assert_eq!(m.valid("+"), true);
        assert_eq!(m.valid("#"), true);
        assert_eq!(m.valid("hello#world"), false);
        assert_eq!(m.valid("hello+world"), false);
        assert_eq!(m.valid("hello/world"), false);
        assert_eq!(m.valid("hello¥world"), false);
        assert_eq!(m.valid("aトピック"), false);
        assert_eq!(m.valid(".-."), true);

        //assert_eq!(2 + 1, 3);
    }

    #[test]
    fn test_aws_example_3() {
        let anyid = "tekitou".to_string();
        let mut m = TopicFilterStore::new();
        let _ = m
            .register_topicfilter(
                SubInfo::new("sensor/+/room1".to_string(), None, anyid.clone()),
                anyid.clone(),
            )
            .unwrap();
        let topic_filter = m
            .get_topicfilter(&"sensor/temperature/room1".to_string())
            .unwrap();

        let expected = vec![SubInfo::new("sensor/+/room1".to_string(), None, anyid)];

        assert_eq!(topic_filter.len(), expected.len());
        assert_eq!(
            topic_filter[0].get_topic_filter(),
            expected[0].get_topic_filter()
        );
    }
}

/*
fn assert_vec_eq<T>(a: Vec<T>, b: Vec<T>) {
    for in a
}
*/
