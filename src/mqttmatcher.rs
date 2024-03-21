use itertools::Itertools;

struct TopicFilterStore {
    elements: Vec<Box<dyn TopicFilter>>,
}

pub struct SubInfo {
    topicfilter_elements: Vec<String>,
}

impl TopicFilter for SubInfo {
    fn get_topic_filter(&self) -> &Vec<String> {
        return &self.topicfilter_elements;
    }
}

impl SubInfo {
    fn new(topicfilter: String) -> Self {
        SubInfo {
            topicfilter_elements: topicfilter.split('/').map(|s| s.to_string()).collect(),
        }
    }
}

pub trait TopicFilter {
    fn get_topic_filter(&self) -> &Vec<String>;
}

impl TopicFilterStore {
    pub fn new() -> Self {
        return TopicFilterStore { elements: vec![] };
    }
    /*
        pub fn register_topicfilter(&mut self, topicfilter: String) -> std::io::Result<()> {
            if topicfilter.split('/').any(|s| !self.valid(s)) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Invalid Topicfilter",
                ));
            }

            let p: Vec<String> = topicfilter.split('/').map(|s| s.to_string()).collect();
            self.elements.push(p);
            Ok(())
        }
    */
    pub fn register_topicfilter<T: TopicFilter + 'static>(
        &mut self,
        topicfilter: T,
    ) -> std::io::Result<()> {
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
        self.elements.push(Box::new(topicfilter));
        Ok(())
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
        let m = TopicFilterStore::new();
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
            .register_topicfilter(SubInfo::new("sensor/#".to_string()))
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
            .register_topicfilter(SubInfo::new("sensor/+/room1".to_string()))
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
}
