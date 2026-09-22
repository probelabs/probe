require "./models/user"

module ProbeFixture
  type Callback = Proc(Int32, Int32)

  module MathTools
    def self.add(left : Int32, right : Int32) : Int32
      left + right
    end

    def self.apply(value : Int32, callback : Callback) : Int32
      callback.call(value)
    end
  end

  lib LibMath
    fun abs(value : Int32) : Int32

    struct NativePoint
      x : Int32
      y : Int32
    end

    union NumericValue
      int_value : Int32
      double_value : Float64
    end
  end
end
