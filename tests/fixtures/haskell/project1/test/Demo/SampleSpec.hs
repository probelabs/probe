module Demo.SampleSpec where

import Test.Hspec
import Demo.Sample

spec :: Spec
spec = describe "Demo.Sample" do
  it "detects active users" do
    active (User 1 "Ada" Admin) `shouldBe` True
