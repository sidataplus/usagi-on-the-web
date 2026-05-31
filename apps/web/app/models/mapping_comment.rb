class MappingComment < Comment
  self.table_name = "comments"
  prefixed_id "com"
end
