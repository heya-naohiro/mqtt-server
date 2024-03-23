use crate::mqttcoder;

use itertools::Itertools;
use std::sync::Arc;
use tokio::sync::Mutex;

use tokio::sync::mpsc;

pub type SubscriptionStore = Arc<Mutex<TopicFilterStore<SubInfo>>>;

pub struct TopicFilterStore<T> {
    elements: Vec<T>,
}

#[derive(Debug)]
pub struct SubInfo {
    pub topicfilter_elements: Vec<String>,
    pub sender: Option<mpsc::Sender<mqttcoder::MQTTPacket>>,
}

impl TopicFilter for SubInfo {
    fn get_topic_filter(&self) -> &Vec<String> {
        return &self.topicfilter_elements;
    }
}

impl SubInfo {
    pub fn new(topicfilter: String, sender: Option<mpsc::Sender<mqttcoder::MQTTPacket>>) -> Self {
        SubInfo {
            topicfilter_elements: topicfilter.split('/').map(|s| s.to_string()).collect(),
            sender,
        }
    }
}
pub trait TopicFilter {
    fn get_topic_filter(&self) -> &Vec<String>;
}

impl<T: TopicFilter> TopicFilterStore<T> {
    pub fn new() -> Self {
        return TopicFilterStore { elements: vec![] };
    }

    pub fn register_topicfilter(&mut self, topicfilter: T) -> std::io::Result<()> {
        println!("regitee!!!!!!!!!");
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
        println!("pushed");
        self.elements.push(topicfilter);
        println!("regitee!!!!!!!!! --- end ");
        Ok(())
    }

    pub fn get_topicfilter(&mut self, topic: &String) -> std::io::Result<Vec<&T>> {
        let mut ret = vec![];
        println!(
            "get topic filter {:?} vs list {:?}",
            topic,
            &self.elements.len()
        );
        for topic_filter in &self.elements {
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
                println!("matcheddd push !!!!");
                ret.push(topic_filter);
            }
            // check next filter
        }
        return Ok(ret);
    }

    pub fn topic_match(&mut self, topic: String) -> std::io::Result<bool> {
        for topic_filter in self.elements.iter() {
            let mut matched = true;
            for eob in topic
                .split('/')
                .into_iter()
                .zip_longest(topic_filter.get_topic_filter())
            {
                match eob {
                    itertools::EitherOrBoth::Both(te, tfe) => {
                        if tfe == "#" {
                            return Ok(true);
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
                return Ok(true);
            }
            // check next filter
        }
        Ok(false)
    }
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
    fn test_aws_example_1() {
        let mut m = TopicFilterStore::new();
        let _ = m
            .register_topicfilter(SubInfo::new("sensor/#".to_string(), None))
            .unwrap();
        assert_eq!(m.topic_match("sensor/".to_string()).unwrap(), true);
        assert_eq!(
            m.topic_match("sensor/temperature".to_string()).unwrap(),
            true
        );
        assert_eq!(
            m.topic_match("sensor/temperature/room1".to_string())
                .unwrap(),
            true
        );
        assert_eq!(m.topic_match("sensor".to_string()).unwrap(), false);
    }
    #[test]
    fn test_aws_example_2() {
        let mut m = TopicFilterStore::new();
        let _ = m
            .register_topicfilter(SubInfo::new("sensor/+/room1".to_string(), None))
            .unwrap();
        assert_eq!(
            m.topic_match("sensor/temperature/room1".to_string())
                .unwrap(),
            true
        );
        assert_eq!(
            m.topic_match("sensor/temperature/room2".to_string())
                .unwrap(),
            false
        );
        assert_eq!(
            m.topic_match("sensor/humidity/room2".to_string()).unwrap(),
            false
        );
    }

    #[test]
    fn test_aws_example_3() {
        let mut m = TopicFilterStore::new();
        let _ = m
            .register_topicfilter(SubInfo::new("sensor/+/room1".to_string(), None))
            .unwrap();
        let topic_filter = m
            .get_topicfilter(&"sensor/temperature/room1".to_string())
            .unwrap();

        let expected = vec![SubInfo::new("sensor/+/room1".to_string(), None)];

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
