module ProbeFixture
  alias UserId = Int64

  annotation Route
  end

  enum Role
    Guest
    Member
    Admin
  end

  struct Address
    getter city : String
    getter zip : String

    def initialize(@city : String, @zip : String)
    end

    def formatted : String
      "#{city} #{zip}"
    end
  end

  abstract class Serializable
    abstract def serialize : String
  end

  class User < Serializable
    getter id : UserId
    getter name : String
    getter role : Role

    def initialize(@id : UserId, @name : String, @role : Role = Role::Member)
    end

    def self.guest(name : String) : User
      new(0_i64, name, Role::Guest)
    end

    def active? : Bool
      role != Role::Guest
    end

    def serialize : String
      "#{id}:#{name}:#{role}"
    end

    macro define_counter(name)
      def {{name.id}}
        1
      end
    end
  end
end
