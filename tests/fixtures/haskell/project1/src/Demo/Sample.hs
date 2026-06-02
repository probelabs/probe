{-# LANGUAGE PatternSynonyms #-}

module Demo.Sample
  ( User(..)
  , UserId
  , Role(..)
  , Serializable(..)
  , active
  , loadUser
  , pattern GuestUser
  ) where

import qualified Data.Text as T

type UserId = Int

data Role = Admin | Guest
  deriving (Eq, Show)

newtype Email = Email T.Text
  deriving (Eq, Show)

data User = User
  { userId :: UserId
  , userName :: T.Text
  , userRole :: Role
  }

class Serializable a where
  serialize :: a -> T.Text
  defaultName :: a -> T.Text
  defaultName _ = "unknown"

instance Serializable User where
  serialize user = userName user

active :: User -> Bool
active user = userRole user /= Guest

loadUser :: UserId -> IO User
loadUser uid = pure (User uid "Ada" Admin)

(<+>) :: [a] -> [a] -> [a]
(<+>) = (++)

(-->) :: Bool -> a -> Maybe a
True --> value = Just value
False --> _ = Nothing

pattern GuestUser :: User
pattern GuestUser = User 0 "guest" Guest
