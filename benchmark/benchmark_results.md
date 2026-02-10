| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `bundle exec ruby -rcurb -rhttpx -rtyphoeus -I../lib/wreq_rb/wreq_rb.rb -rwreq_rb -rhttp -e 'Curl.get(ENV.fetch("URL_TO_TEST")).body'` | 492.0 ± 51.5 | 441.4 | 611.6 | 1.34 ± 0.16 |
| `bundle exec ruby -rcurb -rhttpx -rtyphoeus -I../lib/wreq_rb/wreq_rb.rb -rwreq_rb -rhttp -e 'HTTP.get(ENV.fetch("URL_TO_TEST")).to_s'` | 415.8 ± 21.1 | 392.2 | 544.2 | 1.14 ± 0.09 |
| `bundle exec ruby -rcurb -rhttpx -rtyphoeus -I../lib/wreq_rb/wreq_rb.rb -rwreq_rb -rhttp -e 'HTTPX.get(ENV.fetch("URL_TO_TEST")).body.to_s'` | 426.0 ± 26.4 | 401.8 | 625.2 | 1.16 ± 0.10 |
| `bundle exec ruby -rcurb -rhttpx -rtyphoeus -I../lib/wreq_rb/wreq_rb.rb -rwreq_rb -rhttp -e 'Typhoeus.get(ENV.fetch("URL_TO_TEST")).body'` | 366.3 ± 21.6 | 336.3 | 485.2 | 1.00 |
| `bundle exec ruby -rcurb -rhttpx -rtyphoeus -I../lib/wreq_rb/wreq_rb.rb -rwreq_rb -rhttp -e 'Wreq::HTTP.get(ENV.fetch("URL_TO_TEST")).to_s'` | 374.9 ± 17.7 | 350.2 | 459.8 | 1.02 ± 0.08 |
