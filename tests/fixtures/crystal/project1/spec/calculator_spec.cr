require "../src/calculator"

describe ProbeFixture::MathTools do
  it "adds numbers" do
    ProbeFixture::MathTools.add(2, 3).should eq(5)
  end
end
