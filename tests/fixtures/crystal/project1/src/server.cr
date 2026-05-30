require "./calculator"
require "http/server"

module ProbeFixture
  class Server
    @users = [] of User

    def initialize(@port : Int32)
    end

    def register(user : User) : Nil
      @users << user
    end

    def start : String
      "listening on #{@port}"
    end

    def build_http_server : HTTP::Server
      HTTP::Server.new do |context|
        context.response.print start
      end
    end
  end
end
